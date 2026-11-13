//! Read retained byte relationships; no extraction, model resolution or store writes.
use super::*;
use crate::artifact::{ArtifactDigest, ArtifactId};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadbackRequest {
    pub schema_version: String,
    pub directory: PathBuf,
    pub receipt_sha256: String,
    pub input: Option<PathBuf>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadbackReport {
    pub schema_version: String,
    pub receipt_sha256: String,
    pub intent_sha256: String,
    pub bundle_state: String,
    pub file_count: usize,
    pub source_state: String,
    pub provider_state: String,
    pub review_state: String,
    pub runtime_qualified: bool,
    pub publication_approval: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Run {
    schema_version: String,
    profile: String,
    state: String,
    plan: SonicPlan,
    semantic: SemanticReport,
    source_sha256: String,
    intent_sha256: String,
    normalization_status: Value,
    normalization_diagnostics: Value,
    review_state: String,
    publication_approval: String,
    source_inputs_included: bool,
    model_provider_candidate_payload_included: bool,
    runtime_qualified: bool,
    scope: String,
    files: Vec<local::FileSubject>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct External {
    artifact: Artifact,
    payload_included: bool,
    authority: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lineage {
    schema_version: String,
    external_selected_inputs: Vec<External>,
    produced: Vec<Artifact>,
    review_state: String,
    source_payloads_retained: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Description {
    schema_version: String,
    source_sha256: String,
    source_dna_sha256: String,
    mode: String,
    candidate: DescriptionCandidate,
    input_modality: String,
    review_state: String,
    human_review_required: bool,
    grounding_scope: String,
}
fn sha(value: &str) -> Result<()> {
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "sonic.readback.sha256"
    );
    Ok(())
}
fn artifact(a: &Artifact) -> Result<()> {
    sha(a.digest().value())?;
    let expected = ArtifactId::from_record(
        &ArtifactDigest::sha256(a.digest().value().into()),
        a.format(),
        a.sources(),
    );
    ensure!(a.id() == &expected, "sonic.readback.artifact.record");
    ensure!(
        a.payload().relative_path()
            == Path::new(&format!(
                "objects/sha256/{}/{}",
                &a.digest().value()[..2],
                a.digest().value()
            )),
        "sonic.readback.artifact.path"
    );
    Ok(())
}
fn membership(root: &Path, expected: &BTreeSet<String>) -> Result<()> {
    let mut directories = BTreeSet::new();
    for name in expected {
        let mut path = Path::new(name).parent();
        while let Some(p) = path {
            if !p.as_os_str().is_empty() {
                directories.insert(p.to_string_lossy().into_owned());
            }
            path = p.parent();
        }
    }
    let mut pending = vec![root.to_path_buf()];
    let mut actual = BTreeSet::new();
    let mut visited = 0usize;
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            visited += 1;
            ensure!(
                visited <= expected.len() + directories.len(),
                "sonic.readback.inventory.limit"
            );
            let entry = entry?;
            let path = entry.path();
            let name = path
                .strip_prefix(root)?
                .to_str()
                .context("sonic.readback.name")?
                .replace('\\', "/");
            let kind = entry.file_type()?;
            if kind.is_dir() {
                ensure!(
                    directories.contains(&name),
                    "sonic.readback.directory.unexpected"
                );
                pending.push(path);
            } else {
                ensure!(
                    kind.is_file() && expected.contains(&name) && actual.insert(name),
                    "sonic.readback.member.unexpected_or_symbolic"
                );
            }
        }
    }
    ensure!(actual == *expected, "sonic.readback.inventory.missing");
    Ok(())
}
/// The external digest binds retained bytes, not authenticated authorship. Missing
/// or malformed receipts are errors and never promote partial directories.
pub fn inspect(
    request: &ReadbackRequest,
    token: ProcessCancellationToken,
) -> Result<ReadbackReport> {
    ensure!(
        request.schema_version == "renderflow.sonic-readback-request/v1",
        "sonic.readback.request.schema"
    );
    sha(&request.receipt_sha256)?;
    let root = &request.directory;
    let mut bundle = Capture::new(root, token.clone())?;
    let receipt = bundle.read("sonic-run.json", local::MAX_JSON)?;
    ensure!(
        local::sha256(&receipt) == request.receipt_sha256,
        "sonic.readback.receipt.external_pin"
    );
    let run: Run = local::parse(&receipt)?;
    ensure!(
        run.schema_version == "renderflow.sonic-run/v1"
            && run.profile == PROFILE
            && matches!(run.state.as_str(), "partial" | "complete")
            && run.plan.schema_version == "renderflow.sonic-plan/v1"
            && run.plan.profile == PROFILE
            && run.plan.engine_version == env!("CARGO_PKG_VERSION")
            && run.intent_sha256 == run.plan.intent_sha256
            && run.review_state == "candidate"
            && run.publication_approval == "not_granted"
            && !run.source_inputs_included
            && !run.model_provider_candidate_payload_included
            && !run.runtime_qualified
            && !run.plan.runtime_qualified
            && run.plan.direct_audio_inference == "unsupported_by_this_profile"
            && run.files.len() == 8,
        "sonic.readback.run.contract"
    );
    sha(&run.source_sha256)?;
    sha(&run.intent_sha256)?;
    sha(&run.plan.input_sha256)?;
    let _retained_non_authoritative = (
        &run.normalization_status,
        &run.normalization_diagnostics,
        &run.scope,
    );
    let partial = run.semantic.mode == SemanticMode::Required && run.semantic.status != "produced";
    ensure!(
        partial == (run.state == "partial")
            && run.semantic.mode == run.plan.semantic.mode
            && matches!(
                run.semantic.status.as_str(),
                "disabled"
                    | "blocked"
                    | "not_configured"
                    | "unavailable"
                    | "produced"
                    | "failed"
                    | "invalid"
            )
            && (run.semantic.status != "produced" || run.semantic.execution.is_some()),
        "sonic.readback.semantic.state"
    );
    let mut files = BTreeMap::new();
    let mut previous = None;
    for subject in &run.files {
        sha(&subject.sha256)?;
        ensure!(
            previous.is_none_or(|p: &str| p < subject.path.as_str()),
            "sonic.readback.files.order_or_duplicate"
        );
        previous = Some(subject.path.as_str());
        let bytes = bundle.read(&subject.path, local::MAX_TOTAL as usize)?;
        ensure!(
            bytes.len() as u64 == subject.size_bytes && local::sha256(&bytes) == subject.sha256,
            "sonic.readback.file.changed"
        );
        files.insert(subject.path.clone(), bytes);
    }
    let get = |name: &str| -> Result<&Vec<u8>> {
        files.get(name).context("sonic.readback.member.missing")
    };
    let dna_bytes = get("sonic-dna.json")?;
    let dna: ArtifactDna = local::parse(dna_bytes)?;
    dna.validate()?;
    ensure!(
        dna.source.digest.value == run.source_sha256
            && dna.provenance.configuration_digest.value == run.intent_sha256
            && dna.approval.human_review_required
            && dna.approval.state == crate::dna::DnaReviewState::Candidate,
        "sonic.readback.dna.binding"
    );
    let facts = facts::collect(&dna)?;
    let description: Description = local::parse(get("audio-description.json")?)?;
    ensure!(
        description.schema_version == "renderflow.sonic-description/v1"
            && description.source_sha256 == run.source_sha256
            && description.source_dna_sha256 == local::sha256(dna_bytes)
            && description.input_modality == "structured_evidence"
            && description.review_state == "candidate"
            && description.human_review_required
            && description.mode
                == if run.semantic.status == "produced" {
                    "model_interpretation_of_structured_evidence"
                } else {
                    "deterministic_evidence_render"
                },
        "sonic.readback.description.binding"
    );
    let _grounding_scope = &description.grounding_scope;
    semantic::validate(&description.candidate, &facts)?;
    ensure!(
        get("audio-description.md")?.as_slice()
            == render::markdown(&description.candidate).as_bytes(),
        "sonic.readback.markdown.changed"
    );
    let guidance: Value = local::parse(get("prompt-guidance.json")?)?;
    ensure!(
        guidance["schema_version"] == "renderflow.sonic-prompt-guidance/v1"
            && guidance["source_dna_sha256"] == local::sha256(dna_bytes)
            && guidance["review_required"] == true
            && guidance["direct_imitation_allowed"] == false
            && guidance["legal_clearance_claimed"] == false,
        "sonic.readback.guidance.binding"
    );
    let lineage: Lineage = local::parse(get("lineage.json")?)?;
    ensure!(
        lineage.schema_version == "renderflow.sonic-lineage/v1"
            && lineage.review_state == "candidate"
            && !lineage.source_payloads_retained
            && lineage.external_selected_inputs.len() == run.plan.inputs.len()
            && lineage.produced.len() == 3,
        "sonic.readback.lineage.shape"
    );
    let mut inputs = Vec::new();
    for (external, selected) in lineage
        .external_selected_inputs
        .iter()
        .zip(&run.plan.inputs)
    {
        artifact(&external.artifact)?;
        ensure!(
            !external.payload_included
                && external.authority == "selected_input"
                && external.artifact.storage_class() == ArtifactStorageClass::Source
                && external.artifact.digest().value() == selected.sha256
                && external.artifact.size_bytes() == selected.size_bytes,
            "sonic.readback.external_input.binding"
        );
        inputs.push(external.artifact.id().clone());
    }
    ensure!(
        !inputs.is_empty()
            && run.plan.inputs[0].role == "audio"
            && run.plan.inputs[0].sha256 == run.source_sha256,
        "sonic.readback.source.binding"
    );
    let mut expected = BTreeSet::from([
        "sonic-run.json".into(),
        "sonic-dna.json".into(),
        "audio-description.json".into(),
        "audio-description.md".into(),
        "prompt-guidance.json".into(),
        "lineage.json".into(),
    ]);
    for (index, (a, name)) in lineage
        .produced
        .iter()
        .zip([
            "sonic-dna.json",
            "audio-description.json",
            "prompt-guidance.json",
        ])
        .enumerate()
    {
        artifact(a)?;
        let path = a
            .payload()
            .relative_path()
            .to_str()
            .context("sonic.readback.cas.path")?;
        ensure!(
            a.format().as_str()
                == if index == 0 {
                    crate::dna::ARTIFACT_DNA_FORMAT_V1
                } else {
                    "json"
                },
            "sonic.readback.produced.format"
        );
        ensure!(
            get(path)? == get(name)?
                && a.digest().value() == local::sha256(get(name)?)
                && a.size_bytes() == get(name)?.len() as u64
                && a.storage_class() == ArtifactStorageClass::Terminal,
            "sonic.readback.cas.binding"
        );
        ensure!(
            a.sources()
                == if index == 0 {
                    inputs.as_slice()
                } else {
                    std::slice::from_ref(lineage.produced[0].id())
                },
            "sonic.readback.lineage.parents"
        );
        expected.insert(path.to_owned());
    }
    membership(root, &expected)?;
    let source_state = match &request.input {
        None => "not_supplied",
        Some(input) => match local::read_file(input, local::MAX_JSON) {
            Err(_) => "unavailable",
            Ok(bytes) if local::sha256(&bytes) != run.plan.input_sha256 => "stale",
            Ok(_) => match capture(input, token.clone()) {
                Err(_) => "unavailable_or_changed",
                Ok(current) => {
                    let mut fresh = plan_captured(&current, None)?;
                    fresh.semantic = run.plan.semantic.clone();
                    if serde_json::to_value(&fresh)? == serde_json::to_value(&run.plan)? {
                        "current"
                    } else {
                        "stale"
                    }
                }
            },
        },
    };
    for (name, bytes) in &files {
        ensure!(
            bundle.read(name, local::MAX_TOTAL as usize)? == *bytes,
            "sonic.readback.changed_during_observation"
        );
    }
    ensure!(
        bundle.read("sonic-run.json", local::MAX_JSON)? == receipt,
        "sonic.readback.receipt.changed"
    );
    membership(root, &expected)?;
    bundle.check()?;
    Ok(ReadbackReport {
        schema_version: "renderflow.sonic-readback/v1".into(),
        receipt_sha256: request.receipt_sha256.clone(),
        intent_sha256: run.intent_sha256,
        bundle_state: run.state,
        file_count: run.files.len(),
        source_state: source_state.into(),
        provider_state: "not_reobserved; retained declarations only".into(),
        review_state: "candidate".into(),
        runtime_qualified: false,
        publication_approval: "not_granted".into(),
    })
}
