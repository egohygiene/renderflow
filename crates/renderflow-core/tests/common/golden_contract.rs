//! Additive #412 contracts for the existing golden harness. Authored, UNRUN.
//! This module never probes providers or treats an expected outcome as evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};
use std::time::Instant;

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Corpus, ScenarioEvidence};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Taxonomy {
    pub class: String,
    pub assertion_scope: String,
    pub construction: String,
    pub seed: u64,
    pub generator: String,
    pub residual: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_fixtures: usize,
    pub max_fixture_bytes: usize,
    pub max_total_source_bytes: usize,
    pub cooperative_seconds: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_fixtures: 128, max_fixture_bytes: 65536,
            max_total_source_bytes: 1024 * 1024, cooperative_seconds: 120 }
    }
}

impl Limits {
    pub fn deadline(&self, started: Instant) -> Result<()> {
        ensure!(started.elapsed().as_secs() < self.cooperative_seconds,
            "golden corpus cooperative execution budget exceeded");
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub catalog_path: String,
    pub catalog_sha256: String,
    pub scopes: Vec<Scope>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub id: String,
    pub category: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct CoverageReport {
    pub catalog_path: String,
    pub catalog_sha256: String,
    pub format_registry: &'static str,
    pub runtime_qualified: bool,
    pub scope: &'static str,
    pub rows: Vec<CoverageRow>,
}

#[derive(Debug, Serialize)]
pub struct CoverageRow {
    pub id: String,
    pub category: String,
    pub status: &'static str,
    pub scenarios: Vec<CoverageObservation>,
}

#[derive(Debug, Serialize)]
pub struct CoverageObservation {
    pub id: String,
    pub status: String,
    pub assertion_scope: String,
}

pub fn catalog(config: &Coverage) -> Result<Vec<String>> {
    const PATH: &str = "crates/renderflow-core/data/creative-routes.json";
    ensure!(config.catalog_path == PATH, "coverage must use the shared creative route catalog");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../").join(PATH);
    let bytes = std::fs::read(path)?;
    ensure!(format!("{:x}", Sha256::digest(&bytes)) == config.catalog_sha256,
        "creative catalog changed; explicitly review coverage mapping and update the pin");
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let routes = value["routes"].as_array().context("creative routes must be an array")?;
    let mut ids = BTreeSet::new();
    for route in routes {
        let id = route["id"].as_str().context("creative route ID missing")?;
        ensure!(ids.insert(id.to_string()), "duplicate creative route ID");
    }
    Ok(ids.into_iter().collect())
}

fn format_targets() -> Vec<String> {
    renderflow::graph::capability::FormatCapabilityRegistry::global().all()
        .flat_map(|format| format.capabilities.iter().map(move |operation|
            format!("format.{}.{}", format.id, operation.as_str())))
        .collect()
}

pub fn check(corpus: &Corpus) -> Result<()> {
    let limits = &corpus.limits;
    ensure!(limits.max_fixtures > 0 && limits.max_fixtures <= 1024
        && limits.max_fixture_bytes > 0 && limits.max_fixture_bytes <= 1024 * 1024
        && limits.max_total_source_bytes >= limits.max_fixture_bytes
        && limits.max_total_source_bytes <= 16 * 1024 * 1024
        && (1..=600).contains(&limits.cooperative_seconds), "invalid corpus bounds");
    ensure!(corpus.fixtures.len() <= limits.max_fixtures, "fixture count bound");
    let mut names = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut total = 0usize;
    for fixture in &corpus.fixtures {
        let path = Path::new(&fixture.filename);
        ensure!(path.components().count() == 1
            && path.components().all(|part| matches!(part, Component::Normal(_)))
            && !fixture.filename.chars().any(|ch| matches!(ch, '\\' | ':'))
            && names.insert(fixture.filename.to_ascii_lowercase()),
            "fixture names must be confined and unique on case-insensitive filesystems");
        ensure!(identities.insert(fixture.id.as_str()), "duplicate fixture identity");
        let size = match fixture.encoding.as_str() {
            "utf8" => fixture.payload.len(),
            "hex" => {
                ensure!(fixture.payload.len().is_multiple_of(2)
                    && fixture.payload.bytes().all(|byte| byte.is_ascii_hexdigit()), "invalid hex payload");
                fixture.payload.len() / 2
            }
            _ => anyhow::bail!("unknown fixture encoding"),
        };
        total = total.checked_add(size).context("source size overflow")?;
        ensure!(size <= limits.max_fixture_bytes && total <= limits.max_total_source_bytes,
            "source materialization byte bound");
        if let Some(taxonomy) = &fixture.taxonomy {
            ensure!(matches!(taxonomy.class.as_str(), "valid" | "boundary" | "malformed" | "unsupported" | "resource_stress")
                && taxonomy.assertion_scope == "intake"
                && !taxonomy.construction.is_empty() && taxonomy.seed == 0
                && taxonomy.generator == "renderflow.golden.inline/v1" && !taxonomy.residual.is_empty()
                && fixture.sha256.is_some(), "incomplete classified fixture recipe");
        }
        if let Some(budget) = &fixture.extraction_limits {
            ensure!((1..=4).contains(&budget.max_depth) && (1..=32).contains(&budget.max_artifacts)
                && (1..=65536).contains(&budget.max_extracted_bytes)
                && budget.max_expansion_ratio.is_finite() && (1.0..=16.0).contains(&budget.max_expansion_ratio),
                "adversarial extraction limits must stay small");
        }
    }
    for collection in &corpus.collections {
        ensure!(identities.insert(collection.id.as_str()) && !collection.members.is_empty(),
            "duplicate or empty collection");
        ensure!(collection.members.iter().all(|id| corpus.fixtures.iter().any(|fixture| &fixture.id == id)),
            "unknown collection member");
    }
    let mut scenario_ids = BTreeSet::new();
    let mut coverage_ids = BTreeSet::new();
    if let Some(config) = &corpus.coverage {
        coverage_ids.extend(catalog(config)?);
        coverage_ids.extend(format_targets());
        for scope in &config.scopes {
            ensure!(coverage_ids.insert(scope.id.clone()) && !scope.description.is_empty()
                && matches!(scope.category.as_str(), "input_family" | "failure_class" | "local_capability"),
                "duplicate or incomplete coverage scope");
        }
    }
    for scenario in &corpus.scenarios {
        ensure!(scenario_ids.insert(&scenario.id), "duplicate scenario");
        ensure!(scenario.coverage.iter().all(|id| coverage_ids.contains(id)), "unknown coverage scope");
        ensure!(matches!(scenario.assertion_scope.as_str(), "synthetic_contract" | "intake" | "native_output" | "availability" | "inventory"),
            "unknown scenario assertion scope");
    }
    Ok(())
}

pub fn report(corpus: &Corpus, observed: &[ScenarioEvidence]) -> Result<Option<CoverageReport>> {
    let Some(config) = &corpus.coverage else { return Ok(None); };
    let mut targets = catalog(config)?.into_iter().map(|id| (id, "creative_route".to_string()))
        .collect::<BTreeMap<_, _>>();
    targets.extend(format_targets().into_iter().map(|id| (id, "format_capability".into())));
    targets.extend(config.scopes.iter().map(|scope| (scope.id.clone(), scope.category.clone())));
    let rows = targets.into_iter().map(|(id, category)| {
        let scenarios = corpus.scenarios.iter().filter(|case| case.coverage.contains(&id))
            .map(|case| CoverageObservation {
                id: case.id.clone(), assertion_scope: case.assertion_scope.clone(),
                status: observed.iter().find(|result| result.id == case.id)
                    .map(|result| result.status.clone()).unwrap_or_else(|| "not_executed".into()),
            }).collect::<Vec<_>>();
        let status = if scenarios.is_empty() { "gap" }
            else if scenarios.iter().any(|case| case.status == "passed"
                && matches!(case.assertion_scope.as_str(), "intake" | "native_output")) { "observed_case" }
            else if scenarios.iter().any(|case| case.status == "passed") { "contract_or_inventory_only" }
            else { "not_executed" };
        CoverageRow { id, category, status, scenarios }
    }).collect();
    Ok(Some(CoverageReport { catalog_path: config.catalog_path.clone(), catalog_sha256: config.catalog_sha256.clone(),
        format_registry: "FormatCapabilityRegistry embedded in the reported engine version",
        runtime_qualified: false, scope: "Finite cases only. Intake identification, synthetic contract mocks, provider discovery and native output assertions are different scopes; no row establishes universal format or platform support.", rows }))
}

pub fn default_assertion_scope() -> String { "synthetic_contract".into() }
