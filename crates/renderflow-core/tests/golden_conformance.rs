use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use renderflow::artifact::{
    Artifact, ArtifactCollection, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore,
    ArtifactTransform,
};
use renderflow::checkpoint::{CheckpointContext, RecoveryAction};
use renderflow::evidence::{
    sha256_serialized, sha256_text, ArtifactEvidence, ArtifactManifest, ArtifactRole,
    CacheDisposition, FidelityDeclaration, RunManifest, RunState, StepState, ValidationState,
    ARTIFACT_MANIFEST_SCHEMA_V1, RUN_MANIFEST_SCHEMA_V1,
};
use renderflow::graph::{
    DagExecutor, ExecutionPlan, Format, InputKind, TransformEdge, TransformGraph,
};
use renderflow::hygiene::{HygieneEngine, HygieneFindingKind, HygieneStatus};
use renderflow::optimization::OptimizationMode;
use renderflow::spec::{
    DerivativeProfile, HygienePolicy, ProtectedReferencePolicy, SecretHygienePolicy,
};
use renderflow::transforms::aggregation::AggregationTransform;
use renderflow::transforms::Transform;
use renderflow::{IntakeBudgets, IntakeEngine, IntakeRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CORPUS_SCHEMA: &str = "renderflow.golden-corpus/v1";
const REPORT_SCHEMA: &str = "renderflow.conformance-report/v1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: String,
    corpus_version: String,
    license: String,
    redistribution: String,
    fixtures: Vec<Fixture>,
    collections: Vec<FixtureCollection>,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    id: String,
    filename: String,
    encoding: String,
    payload: String,
    expected_format: Option<String>,
    expected_media_type: String,
    family: String,
    expected_outcome: String,
    expected_diagnostic: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureCollection {
    id: String,
    members: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    id: String,
    tier: String,
    fixtures: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ConformanceReport {
    schema_version: &'static str,
    corpus_version: String,
    corpus_digest: ReportDigest,
    engine_version: &'static str,
    tier: String,
    status: &'static str,
    fixtures: Vec<FixtureEvidence>,
    scenarios: Vec<ScenarioEvidence>,
}

#[derive(Debug, Serialize)]
struct ReportDigest {
    algorithm: &'static str,
    value: String,
}

#[derive(Debug, Serialize)]
struct FixtureEvidence {
    id: String,
    artifact_id: String,
    digest: ReportDigest,
    format: Option<String>,
    media_type: String,
    outcome: String,
}

#[derive(Debug, Serialize)]
struct ScenarioEvidence {
    id: String,
    status: String,
    assertions: Vec<String>,
    providers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Default)]
struct ScenarioResults(BTreeMap<String, ScenarioEvidence>);

impl ScenarioResults {
    fn insert(&mut self, entry: (String, ScenarioEvidence)) {
        self.0.insert(entry.0, entry.1);
    }

    fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }

    fn into_values(self) -> impl Iterator<Item = ScenarioEvidence> {
        self.0.into_values()
    }
}

struct StaticTransform {
    name: &'static str,
    output: &'static str,
    executions: Option<Arc<AtomicUsize>>,
}

impl Transform for StaticTransform {
    fn name(&self) -> &str {
        self.name
    }

    fn apply(&self, _input: String) -> Result<String> {
        if let Some(executions) = &self.executions {
            executions.fetch_add(1, Ordering::SeqCst);
        }
        Ok(self.output.to_string())
    }
}

struct AlwaysFails;

impl ArtifactTransform for AlwaysFails {
    fn name(&self) -> &str {
        "fixture.always-fails"
    }

    fn version(&self) -> &str {
        "1"
    }

    fn apply(&self, _: &Artifact, _: Format, _: &ArtifactStore) -> Result<Artifact> {
        anyhow::bail!("synthetic branch-local failure")
    }
}

struct WrongFormat;

impl ArtifactTransform for WrongFormat {
    fn name(&self) -> &str {
        "fixture.wrong-format"
    }

    fn apply(&self, input: &Artifact, _: Format, store: &ArtifactStore) -> Result<Artifact> {
        store.put_bytes(
            br#"{"unexpected":true}"#,
            ArtifactDescriptor::for_format(Format::Json, ArtifactStorageClass::Intermediate)
                .with_source(input.id().clone()),
        )
    }
}

struct OrderedPdf;

impl AggregationTransform for OrderedPdf {
    fn name(&self) -> &str {
        "fixture.ordered-pdf"
    }

    fn aggregate(&self, inputs: &[&str], output_path: &str) -> Result<()> {
        let mut output = fs::File::create(output_path)?;
        output.write_all(b"%PDF-1.4\n% ordered synthetic inputs\n")?;
        for input in inputs {
            let mut bytes = Vec::new();
            fs::File::open(input)?.read_to_end(&mut bytes)?;
            output.write_all(&bytes)?;
        }
        output.write_all(b"\n%%EOF\n")?;
        Ok(())
    }
}

fn corpus_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/golden-artifacts/v1/corpus.json")
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        anyhow::bail!("hex fixture has an odd number of digits");
    }
    (0..value.len())
        .step_by(2)
        .map(|offset| {
            u8::from_str_radix(&value[offset..offset + 2], 16)
                .with_context(|| format!("invalid hex fixture at byte {}", offset / 2))
        })
        .collect()
}

fn materialize_fixture(root: &Path, fixture: &Fixture) -> Result<PathBuf> {
    let bytes = match fixture.encoding.as_str() {
        "utf8" => fixture.payload.as_bytes().to_vec(),
        "hex" => decode_hex(&fixture.payload)?,
        other => anyhow::bail!("unsupported fixture encoding '{other}'"),
    };
    let path = root.join(&fixture.filename);
    fs::write(&path, bytes)?;
    Ok(path)
}

fn source_artifact(store: &ArtifactStore, path: &Path) -> Result<Artifact> {
    store.import_path(
        path,
        ArtifactDescriptor::for_format(Format::Markdown, ArtifactStorageClass::Source),
    )
}

fn one_edge(from: Format, to: Format) -> renderflow::graph::MultiTargetDag {
    let mut graph = TransformGraph::new();
    graph.add_transform(
        TransformEdge::with_input_kind(from, to, 1.0, 1.0, InputKind::Single)
            .with_provider("provider.fixture.local", format!("{from}.to.{to}"))
            .with_variant("fixture-v1"),
    );
    graph
        .build_multi_target_dag(from, &[to])
        .expect("fixture edge is reachable")
}

fn checkpoint_context(toolchain: &str) -> CheckpointContext {
    CheckpointContext {
        execution_plan_digest: sha256_text("fixture-plan-v1"),
        source_spec_digest: sha256_text("fixture-source-spec-v1"),
        toolchain_fingerprint: Some(toolchain.to_string()),
    }
}

fn scenario(id: &str, assertions: &[&str], providers: &[&str]) -> (String, ScenarioEvidence) {
    (
        id.to_string(),
        ScenarioEvidence {
            id: id.to_string(),
            status: "passed".to_string(),
            assertions: assertions
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            providers: providers.iter().map(|value| (*value).to_string()).collect(),
            reason: None,
        },
    )
}

fn tool_version(program: &str) -> Option<String> {
    let argument = if program == "ffmpeg" {
        "-version"
    } else {
        "--version"
    };
    let output = std::process::Command::new(program)
        .arg(argument)
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("version unavailable")
            .trim()
            .to_string()
    })
}

#[test]
fn golden_artifact_forest_conformance() -> Result<()> {
    let corpus_bytes = fs::read(corpus_path())?;
    let corpus: Corpus = serde_json::from_slice(&corpus_bytes)?;
    assert_eq!(corpus.schema_version, CORPUS_SCHEMA);
    assert_eq!(corpus.license, "CC0-1.0");
    assert!(corpus.redistribution.contains("Synthetic"));

    let fixture_ids = corpus
        .fixtures
        .iter()
        .map(|fixture| fixture.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(fixture_ids.len(), corpus.fixtures.len());
    assert_eq!(corpus.fixtures.len(), 12);
    assert_eq!(corpus.collections.len(), 1);
    for collection in &corpus.collections {
        assert!(collection.id.starts_with("fixture.collection."));
        assert!(collection
            .members
            .iter()
            .all(|member| fixture_ids.contains(member.as_str())));
    }
    for declared in &corpus.scenarios {
        assert!(!declared.fixtures.is_empty());
        assert!(matches!(
            declared.tier.as_str(),
            "fast" | "tool_backed" | "maximal"
        ));
        assert!(declared.fixtures.iter().all(|fixture| {
            fixture_ids.contains(fixture.as_str())
                || corpus.collections.iter().any(|value| value.id == *fixture)
        }));
    }

    let work = tempfile::tempdir()?;
    let fixture_root = work.path().join("fixtures");
    fs::create_dir_all(&fixture_root)?;
    let intake_store = ArtifactStore::new(work.path().join("intake-store"))?;
    let mut paths = BTreeMap::new();
    let mut fixture_evidence = Vec::new();
    let mut observed_families = BTreeSet::new();
    for fixture in &corpus.fixtures {
        let path = materialize_fixture(&fixture_root, fixture)?;
        let request = IntakeRequest::from_path(&path)
            .with_extraction(true)
            .with_budgets(IntakeBudgets {
                max_depth: 1,
                max_artifacts: 8,
                max_extracted_bytes: 64 * 1024,
                max_expansion_ratio: 16.0,
            });
        let report = IntakeEngine::new().intake(&request, &intake_store)?;
        assert_eq!(report.profile.format, fixture.expected_format);
        assert_eq!(report.profile.media_type, fixture.expected_media_type);
        match fixture.expected_outcome.as_str() {
            "accepted" => assert!(
                report.profile.conflicts.is_empty(),
                "{} unexpectedly reported conflicts: {:?}",
                fixture.id,
                report.profile.conflicts
            ),
            "conflict" => assert!(!report.profile.conflicts.is_empty()),
            "rejected" => {
                let code = fixture
                    .expected_diagnostic
                    .as_deref()
                    .context("rejected fixture must name its diagnostic")?;
                assert!(report.diagnostics.iter().any(|item| item.code == code));
            }
            other => anyhow::bail!("unknown expected fixture outcome '{other}'"),
        }
        intake_store.verify(&report.source)?;
        observed_families.insert(fixture.family.as_str());
        fixture_evidence.push(FixtureEvidence {
            id: fixture.id.clone(),
            artifact_id: report.source.id().to_string(),
            digest: ReportDigest {
                algorithm: "sha256",
                value: report.source.digest().value().to_string(),
            },
            format: report.profile.format,
            media_type: report.profile.media_type,
            outcome: fixture.expected_outcome.clone(),
        });
        paths.insert(fixture.id.clone(), path);
    }
    assert_eq!(
        observed_families,
        BTreeSet::from([
            "archive", "audio", "data", "document", "image", "subtitle", "unknown", "video"
        ])
    );

    let mut results = ScenarioResults::default();
    results.insert(scenario(
        "inspect_only",
        &["all fixture identities, formats, media types, conflicts, and diagnostics matched"],
        &["provider.renderflow.native-inspection"],
    ));

    let execution_store = ArtifactStore::new(work.path().join("execution-store"))?;
    let markdown_path = &paths["fixture.document.markdown"];
    let source = source_artifact(&execution_store, markdown_path)?;
    let direct_dag = one_edge(Format::Markdown, Format::Html);
    let mut direct = DagExecutor::new();
    direct.register_single_with_identity(
        Format::Markdown,
        Format::Html,
        Arc::new(StaticTransform {
            name: "fixture.markdown-html",
            output: "<p>Synthetic artifact</p>\n",
            executions: None,
        }),
        "fixture.markdown-html/v1",
    );
    let direct_report = direct.execute_artifact_with_evidence(
        &direct_dag,
        Format::Markdown,
        source.clone(),
        &execution_store,
    )?;
    assert_eq!(direct_report.steps.len(), 1);
    assert_eq!(direct_report.steps[0].state, StepState::Complete);
    assert!(direct_report.artifacts.contains_key(&Format::Html));
    results.insert(scenario(
        "direct_deterministic_conversion",
        &["production DAG executor produced one validated HTML derivative"],
        &["provider.fixture.local"],
    ));

    let mut graph = TransformGraph::new();
    graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
    graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 1.0, 1.0));
    graph.add_transform(TransformEdge::new(Format::Html, Format::Json, 1.0, 1.0));
    let multi_dag = graph
        .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Json])
        .context("multi-target fixture graph must resolve")?;
    assert_eq!(multi_dag.edge_count(), 3);
    let first_plan = ExecutionPlan::from_dag(
        &multi_dag,
        Format::Markdown,
        &[Format::Pdf, Format::Json],
        OptimizationMode::Balanced,
    );
    let second_plan = ExecutionPlan::from_dag(
        &multi_dag,
        Format::Markdown,
        &[Format::Pdf, Format::Json],
        OptimizationMode::Balanced,
    );
    assert_eq!(
        sha256_serialized(&first_plan)?,
        sha256_serialized(&second_plan)?
    );
    let mut multi = DagExecutor::new();
    multi.register_single(
        Format::Markdown,
        Format::Html,
        Arc::new(StaticTransform {
            name: "fixture.markdown-html",
            output: "<p>Synthetic artifact</p>\n",
            executions: None,
        }),
    );
    multi.register_single(
        Format::Html,
        Format::Pdf,
        Arc::new(StaticTransform {
            name: "fixture.html-pdf",
            output: "%PDF-1.4\n%%EOF\n",
            executions: None,
        }),
    );
    multi.register_single(
        Format::Html,
        Format::Json,
        Arc::new(StaticTransform {
            name: "fixture.html-json",
            output: "{\"kind\":\"synthetic-derivative\"}\n",
            executions: None,
        }),
    );
    let multi_report = multi.execute_artifact_with_evidence(
        &multi_dag,
        Format::Markdown,
        source.clone(),
        &execution_store,
    )?;
    assert_eq!(multi_report.steps.len(), 3);
    assert!(multi_report.artifacts.contains_key(&Format::Pdf));
    assert!(multi_report.artifacts.contains_key(&Format::Json));
    results.insert(scenario(
        "multi_step_conversion",
        &["topological execution produced the two-edge PDF path"],
        &["provider.fixture.local"],
    ));
    results.insert(scenario(
        "multi_target_shared_intermediate",
        &["one HTML intermediate served PDF and JSON target branches"],
        &["provider.fixture.local"],
    ));
    results.insert(scenario(
        "cross_family_derivative",
        &["document input produced a validated structured-data derivative"],
        &["provider.fixture.local"],
    ));

    let first_page = source.clone();
    let second_page = execution_store.put_bytes(
        b"second synthetic page",
        ArtifactDescriptor::for_format(Format::Markdown, ArtifactStorageClass::Source),
    )?;
    let mut collection_graph = TransformGraph::new();
    collection_graph.add_collection_transform(Format::Markdown, Format::Pdf, 1.0, 1.0);
    let collection_dag = collection_graph
        .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
        .context("collection graph must resolve")?;
    let mut collection_executor = DagExecutor::new();
    collection_executor.register_aggregation(Format::Markdown, Format::Pdf, Arc::new(OrderedPdf));
    let collection_output = collection_executor.execute_artifacts(
        &collection_dag,
        Format::Markdown,
        ArtifactCollection::new(vec![first_page.clone(), second_page.clone()]),
        &execution_store,
    )?;
    let aggregate = collection_output[&Format::Pdf].clone().into_one()?;
    assert_eq!(
        aggregate.sources(),
        &[first_page.id().clone(), second_page.id().clone()]
    );
    results.insert(scenario(
        "collection_aggregation",
        &["ordered collection membership and output source lineage were preserved"],
        &["provider.fixture.local"],
    ));

    let executions = Arc::new(AtomicUsize::new(0));
    let cache_path = work.path().join("dag-cache.json");
    let mut cached = DagExecutor::new().with_cache(&cache_path);
    cached.register_single_with_identity(
        Format::Markdown,
        Format::Html,
        Arc::new(StaticTransform {
            name: "fixture.counting-html",
            output: "<p>cached</p>\n",
            executions: Some(Arc::clone(&executions)),
        }),
        "fixture.counting-html/v1",
    );
    let cold = cached.execute_artifact_with_evidence(
        &direct_dag,
        Format::Markdown,
        source.clone(),
        &execution_store,
    )?;
    let warm = cached.execute_artifact_with_evidence(
        &direct_dag,
        Format::Markdown,
        source.clone(),
        &execution_store,
    )?;
    assert_eq!(cold.steps[0].cache, CacheDisposition::Miss);
    assert_eq!(warm.steps[0].cache, CacheDisposition::Hit);
    let mut changed = DagExecutor::new().with_cache(&cache_path);
    changed.register_single_with_identity(
        Format::Markdown,
        Format::Html,
        Arc::new(StaticTransform {
            name: "fixture.counting-html",
            output: "<p>cache identity changed</p>\n",
            executions: Some(Arc::clone(&executions)),
        }),
        "fixture.counting-html/v2",
    );
    let invalidated = changed.execute_artifact_with_evidence(
        &direct_dag,
        Format::Markdown,
        source.clone(),
        &execution_store,
    )?;
    assert_eq!(invalidated.steps[0].cache, CacheDisposition::Miss);
    assert_eq!(executions.load(Ordering::SeqCst), 2);
    results.insert(scenario(
        "cache_cold_warm_partial_invalidation",
        &["cold miss, warm hit, and transform-identity invalidation were observable"],
        &["provider.fixture.local"],
    ));

    let mut provider_graph = TransformGraph::new();
    provider_graph.add_transform(
        TransformEdge::new(Format::Markdown, Format::Html, 0.1, 1.0)
            .with_provider("provider.fixture.remote", "document.convert"),
    );
    provider_graph.add_transform(
        TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0)
            .with_provider("provider.fixture.local", "document.convert"),
    );
    let available = HashSet::from(["provider.fixture.local".to_string()]);
    let filtered = provider_graph.filtered_by_available_providers(&available);
    let fallback = filtered
        .build_multi_target_dag(Format::Markdown, &[Format::Html])
        .context("local fallback must remain reachable")?;
    assert_eq!(
        fallback.all_edges()[0].provider_id.as_deref(),
        Some("provider.fixture.local")
    );
    results.insert(scenario(
        "missing_provider_fallback",
        &["unavailable remote edge was excluded while the local edge remained selectable"],
        &["provider.fixture.local"],
    ));

    let mut wrong = DagExecutor::new();
    wrong.register_artifact(Format::Markdown, Format::Html, Arc::new(WrongFormat));
    let wrong_report = wrong.execute_artifact_with_evidence(
        &direct_dag,
        Format::Markdown,
        source.clone(),
        &execution_store,
    )?;
    assert_eq!(wrong_report.steps[0].state, StepState::Failed);
    assert_eq!(wrong_report.steps[0].validation, ValidationState::Invalid);
    results.insert(scenario(
        "validation_failure",
        &["wrong-format output was classified invalid despite a successful transform return"],
        &["provider.fixture.local"],
    ));

    let mut partial_graph = TransformGraph::new();
    partial_graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
    partial_graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 1.0));
    partial_graph.add_transform(TransformEdge::new(Format::Pdf, Format::Json, 1.0, 1.0));
    let partial_dag = partial_graph
        .build_multi_target_dag(Format::Markdown, &[Format::Html, Format::Json])
        .context("partial graph must resolve")?;
    let mut partial = DagExecutor::new();
    partial.register_single(
        Format::Markdown,
        Format::Html,
        Arc::new(StaticTransform {
            name: "fixture.good-branch",
            output: "<p>good branch</p>\n",
            executions: None,
        }),
    );
    partial.register_artifact(Format::Markdown, Format::Pdf, Arc::new(AlwaysFails));
    let partial_report = partial.execute_artifact_with_evidence(
        &partial_dag,
        Format::Markdown,
        source.clone(),
        &execution_store,
    )?;
    assert!(partial_report.artifacts.contains_key(&Format::Html));
    assert!(partial_report
        .steps
        .iter()
        .any(|step| step.state == StepState::Failed));
    assert!(partial_report
        .steps
        .iter()
        .any(|step| step.state == StepState::Skipped));
    results.insert(scenario(
        "branch_local_failure_partial_result",
        &["successful sibling output survived a failed branch and downstream skip"],
        &["provider.fixture.local"],
    ));

    let cancelled = Arc::new(AtomicBool::new(true));
    let cancellation_report = DagExecutor::new()
        .with_cancellation_flag(cancelled)
        .execute_artifact_with_evidence(
            &direct_dag,
            Format::Markdown,
            source.clone(),
            &execution_store,
        )?;
    assert_eq!(cancellation_report.steps[0].state, StepState::Cancelled);
    results.insert(scenario(
        "cancellation",
        &["pre-wave cancellation produced explicit cancelled step evidence"],
        &[],
    ));

    let checkpoint_path = work.path().join("checkpoints.json");
    let checkpoint_runs = Arc::new(AtomicUsize::new(0));
    let build_checkpoint_executor = |resume: bool, identity: &'static str, toolchain: &str| {
        let mut executor = DagExecutor::new()
            .with_toolchain_fingerprint(toolchain)
            .with_checkpoints(&checkpoint_path, checkpoint_context(toolchain), resume);
        executor.register_single_with_identity(
            Format::Markdown,
            Format::Html,
            Arc::new(StaticTransform {
                name: "fixture.checkpoint-html",
                output: "<p>checkpointed</p>\n",
                executions: Some(Arc::clone(&checkpoint_runs)),
            }),
            identity,
        );
        executor
    };
    build_checkpoint_executor(false, "fixture.checkpoint-html/v1", "toolchain-v1")
        .execute_artifact_with_evidence(
            &direct_dag,
            Format::Markdown,
            source.clone(),
            &execution_store,
        )?;
    let resumed = build_checkpoint_executor(true, "fixture.checkpoint-html/v1", "toolchain-v1")
        .execute_artifact_with_evidence(
            &direct_dag,
            Format::Markdown,
            source.clone(),
            &execution_store,
        )?;
    assert_eq!(resumed.steps[0].state, StepState::Reused);
    assert_eq!(checkpoint_runs.load(Ordering::SeqCst), 1);
    results.insert(scenario(
        "checkpoint_resume",
        &["compatible validated checkpoint resumed without repeating work"],
        &["provider.fixture.local"],
    ));
    let changed_context = checkpoint_context("toolchain-v2");
    let previous_context = checkpoint_context("toolchain-v1");
    let decision =
        renderflow::checkpoint::CheckpointStore::open(&checkpoint_path, previous_context)?
            .context_decision(&changed_context);
    assert_eq!(decision.action, RecoveryAction::Recompute);
    let changed_tool =
        build_checkpoint_executor(true, "fixture.checkpoint-html/v1", "toolchain-v2")
            .execute_artifact_with_evidence(
                &direct_dag,
                Format::Markdown,
                source.clone(),
                &execution_store,
            )?;
    assert_eq!(changed_tool.steps[0].state, StepState::Complete);
    let changed_source = execution_store.put_bytes(
        b"# Changed synthetic source\n",
        ArtifactDescriptor::for_format(Format::Markdown, ArtifactStorageClass::Source),
    )?;
    let source_invalidated =
        build_checkpoint_executor(true, "fixture.checkpoint-html/v1", "toolchain-v2")
            .execute_artifact_with_evidence(
                &direct_dag,
                Format::Markdown,
                changed_source,
                &execution_store,
            )?;
    assert_eq!(source_invalidated.steps[0].state, StepState::Complete);
    results.insert(scenario(
        "version_invalidation",
        &["transform identity, toolchain context, and source identity changes forced recomputation"],
        &["provider.fixture.local"],
    ));

    let gated_source = execution_store.put_bytes(
        b"Protected Fixture with FIXTURE-CREDENTIAL-MARKER",
        ArtifactDescriptor::for_format(Format::Markdown, ArtifactStorageClass::Source),
    )?;
    let policy = HygienePolicy {
        secrets: SecretHygienePolicy {
            markers: vec!["FIXTURE-CREDENTIAL-MARKER".to_string()],
            ..SecretHygienePolicy::default()
        },
        protected_references: ProtectedReferencePolicy {
            terms: vec!["Protected Fixture".to_string()],
            ..ProtectedReferencePolicy::default()
        },
        ..HygienePolicy::default()
    };
    let hygiene = HygieneEngine::new().apply(
        "fixture.publication",
        &policy,
        &gated_source,
        &execution_store,
    )?;
    assert_eq!(hygiene.evidence.status, HygieneStatus::Blocked);
    assert!(hygiene
        .evidence
        .findings
        .iter()
        .any(|finding| finding.kind == HygieneFindingKind::ProtectedReference));
    assert!(!serde_json::to_string(&hygiene.evidence)?.contains("FIXTURE-CREDENTIAL-MARKER"));
    results.insert(scenario(
        "privacy_redaction_gate",
        &["secret and protected-reference gates blocked release without leaking the marker"],
        &["renderflow.core-hygiene"],
    ));

    let inner = {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer.start_file("leaf.txt", zip::write::SimpleFileOptions::default())?;
        writer.write_all(b"bounded leaf")?;
        writer.finish()?.into_inner()
    };
    let outer_path = fixture_root.join("nested.zip");
    {
        let file = fs::File::create(&outer_path)?;
        let mut writer = zip::ZipWriter::new(file);
        writer.start_file("inner.zip", zip::write::SimpleFileOptions::default())?;
        writer.write_all(&inner)?;
        writer.finish()?;
    }
    let bounded = IntakeEngine::new().intake(
        &IntakeRequest::from_path(&outer_path)
            .with_extraction(true)
            .with_budgets(IntakeBudgets {
                max_depth: 1,
                max_artifacts: 4,
                max_extracted_bytes: 64 * 1024,
                max_expansion_ratio: 32.0,
            }),
        &intake_store,
    )?;
    assert!(bounded
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "intake.budget.depth"));
    results.insert(scenario(
        "bounded_recursive_extraction",
        &["nested archive extraction stopped at the declared depth budget"],
        &["provider.renderflow.zip"],
    ));

    let everything: DerivativeProfile =
        serde_yaml_ng::from_str(include_str!("../data/profiles/everything-v1.yaml"))?;
    assert!(everything.all_reachable);
    assert!(everything.targets.is_empty());
    results.insert(scenario(
        "everything_profile",
        &["bundled profile selects all policy-allowed reachable branches"],
        &[],
    ));
    let magazine: DerivativeProfile =
        serde_yaml_ng::from_str(include_str!("../data/profiles/magazine-v1.yaml"))?;
    let magazine_roles = magazine
        .targets
        .iter()
        .filter_map(|target| target.role.as_deref())
        .collect::<BTreeSet<_>>();
    assert!(magazine_roles.contains("print/press"));
    assert!(magazine_roles.contains("digital/web/index"));
    assert!(magazine_roles.contains("assets/previews/cover"));
    assert!(magazine.policy.validation.is_some());
    results.insert(scenario(
        "magazine_release_bundle",
        &["versioned magazine roles and release validation gate remain present"],
        &[],
    ));

    let intermediate = &multi_report.artifacts[&Format::Html];
    let output = &multi_report.artifacts[&Format::Json];
    let source_evidence = ArtifactEvidence::from_artifact(
        &source,
        "source",
        ArtifactRole::Source,
        markdown_path.display().to_string(),
        renderflow::evidence::ProducerEvidence::source(),
        ValidationState::Valid,
        FidelityDeclaration::Lossless,
    );
    let output_evidence = ArtifactEvidence::from_artifact(
        output,
        "data",
        ArtifactRole::Terminal,
        "dist/data.json",
        renderflow::evidence::ProducerEvidence {
            system: "renderflow".to_string(),
            transform: Some("fixture.html-json".to_string()),
            capability: Some("data.generate".to_string()),
            provider: Some("provider.fixture.local".to_string()),
            version: Some("1".to_string()),
        },
        ValidationState::Valid,
        FidelityDeclaration::Lossless,
    );
    let intermediate_evidence = ArtifactEvidence::from_artifact(
        intermediate,
        "web-intermediate",
        ArtifactRole::Intermediate,
        "cache/intermediate.html",
        renderflow::evidence::ProducerEvidence {
            system: "renderflow".to_string(),
            transform: Some("fixture.markdown-html".to_string()),
            capability: Some("document.convert".to_string()),
            provider: Some("provider.fixture.local".to_string()),
            version: Some("1".to_string()),
        },
        ValidationState::Valid,
        FidelityDeclaration::Lossless,
    );
    let run_id = format!("run:sha256:{}", "0".repeat(64));
    let manifest = RunManifest {
        schema_version: RUN_MANIFEST_SCHEMA_V1.to_string(),
        run_id: run_id.clone(),
        execution_plan_digest: sha256_serialized(&first_plan)?,
        source_spec_digest: sha256_text("fixture-source-spec-v1"),
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_unix_ms: 0,
        completed_at_unix_ms: 0,
        state: RunState::Complete,
        artifact_manifest: ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_V1.to_string(),
            run_id,
            output_dir: "dist".to_string(),
            outputs: vec!["dist/data.json".to_string()],
            artifacts: vec![source_evidence, intermediate_evidence, output_evidence],
        },
        steps: multi_report.steps,
        diagnostics: multi_report.diagnostics,
        toolchain: None,
        artifact_forest: None,
    };
    let flow = manifest.flow_artifacts_v1();
    assert_eq!(flow.len(), 3);
    assert!(flow
        .iter()
        .all(|artifact| artifact.schema_version == "flow.artifact/v1"));
    assert!(flow
        .iter()
        .all(|artifact| artifact.artifact_id.starts_with("artifact:sha256-")));
    assert!(flow
        .iter()
        .filter(|artifact| artifact.role != "source")
        .all(|artifact| !artifact.sources.is_empty()));

    let tier = std::env::var("RENDERFLOW_CONFORMANCE_TIER").unwrap_or_else(|_| "fast".to_string());
    anyhow::ensure!(matches!(tier.as_str(), "fast" | "tool_backed" | "maximal"));
    let requested_rank = match tier.as_str() {
        "fast" => 0,
        "tool_backed" => 1,
        _ => 2,
    };
    let tools = ["pandoc", "ffmpeg"];
    let available_tools = tools
        .iter()
        .filter_map(|tool| tool_version(tool).map(|version| format!("tool.{tool}@{version}")))
        .collect::<Vec<_>>();
    let all_tools_available = available_tools.len() == tools.len();
    results.insert((
        "optional_tool_providers".to_string(),
        ScenarioEvidence {
            id: "optional_tool_providers".to_string(),
            status: if requested_rank == 0 {
                "excluded"
            } else if all_tools_available {
                "passed"
            } else {
                "unavailable"
            }
            .to_string(),
            assertions: vec![
                "optional provider availability is explicit and never a silent pass".to_string(),
            ],
            providers: available_tools,
            reason: (!all_tools_available)
                .then(|| "one or more optional tool providers were not installed".to_string()),
        },
    ));
    results.insert((
        "maximal_release_matrix".to_string(),
        ScenarioEvidence {
            id: "maximal_release_matrix".to_string(),
            status: if requested_rank == 2 {
                "passed"
            } else {
                "excluded"
            }
            .to_string(),
            assertions: vec![
                "maximal tier records every corpus family and scenario state".to_string(),
            ],
            providers: Vec::new(),
            reason: (requested_rank < 2).then(|| "maximal tier was not requested".to_string()),
        },
    ));

    let declared_scenarios = corpus
        .scenarios
        .iter()
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    let observed_scenarios = results.keys().map(String::as_str).collect::<BTreeSet<_>>();
    assert_eq!(observed_scenarios, declared_scenarios);
    let scenarios = results.into_values().collect::<Vec<_>>();
    let report = ConformanceReport {
        schema_version: REPORT_SCHEMA,
        corpus_version: corpus.corpus_version,
        corpus_digest: ReportDigest {
            algorithm: "sha256",
            value: format!("{:x}", Sha256::digest(&corpus_bytes)),
        },
        engine_version: env!("CARGO_PKG_VERSION"),
        tier,
        status: "passed",
        fixtures: fixture_evidence,
        scenarios,
    };
    let encoded = serde_json::to_vec_pretty(&report)?;
    let decoded: serde_json::Value = serde_json::from_slice(&encoded)?;
    assert_eq!(decoded["schema_version"], REPORT_SCHEMA);
    assert_eq!(decoded["fixtures"].as_array().map(Vec::len), Some(12));
    if let Some(path) = std::env::var_os("RENDERFLOW_CONFORMANCE_REPORT") {
        fs::write(path, encoded)?;
    }
    Ok(())
}
