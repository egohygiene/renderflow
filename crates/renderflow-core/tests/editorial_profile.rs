//! #379 regressions, authored and UNRUN during the maintained sprint deferral.
//! Native cases are explicit opt-ins; a skipped case is not provider evidence.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use renderflow::adapters::strategy::StrategyArtifactTransform;
use renderflow::artifact::{ArtifactDescriptor, ArtifactStorageClass, ArtifactStore, ArtifactTransform};
use renderflow::config::OutputType;
use renderflow::evidence::RunState;
use renderflow::graph::Format;
use renderflow::spec::load_spec_str;
use renderflow::strategies::editorial::{bundle_fingerprint, EDITORIAL_PRESET};
use renderflow::strategies::select_strategy;
use renderflow::{EngineBuilder, ExecutionRequest};

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/editorial")
}

fn spec(targets: &str) -> String {
    format!("schema: renderflow/v2\nsources:\n  - id: article\n    path: article.md\n    format: markdown\n{targets}\noutput:\n  bundle_root: output\n")
}

#[test]
fn yaml_selects_the_same_bundle_and_all_native_target_identities() {
    let loaded = load_spec_str(&spec("targets:\n  profiles: [editorial]")).unwrap();
    let profile = &loaded.spec.profiles["editorial"];
    assert_eq!(profile.targets.len(), 5);
    for format in ["html", "pdf", "docx", "epub"] {
        let target = profile.targets.iter().find(|target| target.format.as_deref() == Some(format)).unwrap();
        assert_eq!(target.preset.as_deref(), Some(EDITORIAL_PRESET));
        assert_eq!(target.transform.as_deref(), Some(format!("builtin.markdown.{format}").as_str()));
    }
    let original = profile.targets.iter().find(|target| target.format.as_deref() == Some("markdown")).unwrap();
    assert!(original.preset.is_none());
    assert_eq!(original.role.as_deref(), Some("source/article"));
}

#[test]
fn local_profiles_keep_precedence_and_can_inherit_the_bundle() {
    let local = "profiles:\n  editorial:\n    targets:\n      - format: markdown\ntargets:\n  profiles: [editorial]";
    assert_eq!(load_spec_str(&spec(local)).unwrap().spec.profiles["editorial"].targets.len(), 1);
    let inherited = "profiles:\n  article.web:\n    extends: [editorial]\n    exclude:\n      roles: [print/article, source/article, editorial/article, ebook/article]\ntargets:\n  profiles: [article.web]";
    let loaded = load_spec_str(&spec(inherited)).unwrap();
    assert!(loaded.spec.profiles.contains_key("editorial"));
    assert!(load_spec_str(&spec("targets:\n  profiles: [unknown.article]")).is_err());
}

#[test]
fn editorial_preset_refuses_ambiguous_routes_and_disables_external_asset_reuse() {
    let transform = StrategyArtifactTransform::new(Format::Markdown, Format::Html, None,
        Some(EDITORIAL_PRESET.to_string()), BTreeMap::new(), None).unwrap();
    assert!(!transform.cacheable());
    assert!(transform.cache_identity().contains(&bundle_fingerprint()));
    assert!(StrategyArtifactTransform::new(Format::Html, Format::Pdf, None,
        Some(EDITORIAL_PRESET.to_string()), BTreeMap::new(), None).is_err());
    assert!(select_strategy(&OutputType::Html, Some("custom.html"), "unused", Some(EDITORIAL_PRESET)).is_err());
    assert!(StrategyArtifactTransform::new(Format::Markdown, Format::Html, None,
        Some(EDITORIAL_PRESET.to_string()), BTreeMap::from([("title".to_string(), "override".to_string())]), None).is_err());
}

fn copy_fixture(root: &Path) {
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::copy(example().join("article.md"), root.join("article.md")).unwrap();
    fs::copy(example().join("assets/garden.png"), root.join("assets/garden.png")).unwrap();
}

#[test]
#[ignore = "requires explicitly resumed qualification and local Pandoc >=3.6.1"]
fn public_sdk_publishes_a_self_contained_article_and_preserves_source() {
    let root = tempfile::tempdir().unwrap();
    copy_fixture(root.path());
    let source = fs::read(root.path().join("article.md")).unwrap();
    let config = spec("targets:\n  exact:\n    - id: article.web\n      role: web/article\n      format: html\n      transform: builtin.markdown.html\n      preset: editorial-v1");
    fs::write(root.path().join("renderflow.yaml"), config).unwrap();
    let result = EngineBuilder::new().build().execute(ExecutionRequest::from_path(root.path().join("renderflow.yaml"))).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete);
    assert_eq!(result.run_manifest.published_targets().unwrap().len(), 1);
    let html = fs::read_to_string(root.path().join("output/article/web/article.html")).unwrap();
    assert!(html.contains("article-title") && html.contains("<figcaption") && html.contains("<nav"));
    assert!(html.contains("data:image/png") && html.contains("@font-face") && html.contains("DejaVu Serif"));
    assert!(html.contains("{{ observation }}"));
    assert!(!html.contains("file://") && !html.contains("assets/garden.png"));
    assert_eq!(fs::read(root.path().join("article.md")).unwrap(), source);
}

#[test]
#[ignore = "requires explicitly resumed qualification and local Pandoc >=3.6.1"]
fn invalid_article_content_does_not_commit_a_derivative() {
    let root = tempfile::tempdir().unwrap();
    let store = ArtifactStore::new(root.path().join("store")).unwrap();
    let header = "---\narticle-schema: renderflow.editorial/v1\ntitle: Example\nlang: en-US\n";
    for text in [
        format!("{header}date: \"2026-02-30\"\n---\n## Body\n"),
        format!("{header}unknown: value\n---\n## Body\n"),
        format!("{header}---\n# Repeated title\n"),
        format!("{header}---\n## Body\n\n![Remote](https://example.invalid/image.png)\n"),
        format!("{header}---\n## Body\n\n<script>alert(1)</script>\n"),
        format!("{header}---\n## Body\n\n![Missing](missing.png)\n"),
    ] {
        let source = store.put_bytes(text.as_bytes(), ArtifactDescriptor::for_format(Format::Markdown, ArtifactStorageClass::Source)).unwrap();
        let transform = StrategyArtifactTransform::new(Format::Markdown, Format::Html, None,
            Some(EDITORIAL_PRESET.to_string()), BTreeMap::new(), Some(root.path().to_path_buf())).unwrap();
        assert!(transform.apply(&source, Format::Html, &store).is_err());
        assert_eq!(store.read_bytes(&source).unwrap(), text.as_bytes());
    }
}

#[test]
#[ignore = "requires resumed qualification, Pandoc >=3.6.1 and a prepared offline Tectonic cache"]
fn full_profile_publishes_real_derivatives_and_the_exact_original() {
    let root = tempfile::tempdir().unwrap();
    copy_fixture(root.path());
    fs::copy(example().join("renderflow.yaml"), root.path().join("renderflow.yaml")).unwrap();
    let source = fs::read(root.path().join("article.md")).unwrap();
    let result = EngineBuilder::new().build().execute(ExecutionRequest::from_path(root.path().join("renderflow.yaml"))).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete);
    assert_eq!(result.run_manifest.published_targets().unwrap().len(), 5);
    assert!(fs::read(root.path().join("output/article/print/article.pdf")).unwrap().starts_with(b"%PDF-"));
    assert!(fs::read(root.path().join("output/article/editorial/article.docx")).unwrap().starts_with(b"PK"));
    assert!(fs::read(root.path().join("output/article/ebook/article.epub")).unwrap().starts_with(b"PK"));
    assert_eq!(fs::read(root.path().join("output/article/source/article.markdown")).unwrap(), source);
    assert_eq!(fs::read(root.path().join("article.md")).unwrap(), source);
}
