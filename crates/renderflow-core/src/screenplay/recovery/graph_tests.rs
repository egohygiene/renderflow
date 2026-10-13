//! Authored regression cases; runtime qualification is tracked separately.

use super::*;

const SOURCE: &[u8] = b"INT. LAB - DAY\n\nA light blinks.\n";

fn policy() -> RecoveryExtractionConfig {
    RecoveryExtractionConfig { allow_unqualified: true, ..RecoveryExtractionConfig::default() }
}

fn setup() -> (tempfile::TempDir, ArtifactStore, Artifact, Artifact) {
    let directory = tempfile::tempdir().unwrap();
    let store = ArtifactStore::new(directory.path()).unwrap();
    let source = store.put_bytes(SOURCE,
        ArtifactDescriptor::for_format(Format::Text, ArtifactStorageClass::Source)).unwrap();
    let canonical = RecoveryArtifactTransform::new(policy(), None).unwrap()
        .apply(&source, Format::ScreenplayJson, &store).unwrap();
    (directory, store, source, canonical)
}

fn alter_metadata(artifact: &Artifact, alter: impl FnOnce(&mut serde_json::Value)) -> Artifact {
    let mut value = serde_json::to_value(artifact).unwrap();
    alter(&mut value["metadata"][SCREENPLAY_RECOVERY_ARTIFACT_METADATA_KEY]);
    serde_json::from_value(value).unwrap()
}

#[test]
fn plain_text_recovery_retains_pages_and_is_always_review_required() {
    let (_directory, store, source, canonical) = setup();
    validate_published_artifact(&canonical, &source, &store, &policy()).unwrap();
    let evidence = read_evidence(&canonical).unwrap();
    assert_eq!(evidence.candidate.pages[0].text.as_bytes(), SOURCE);
    assert_eq!(evidence.original_source.id(), source.id());
    assert_eq!(canonical.sources(), [source.id().clone()]);
    assert_eq!(evidence.input_artifact_id, *source.id());
    assert_eq!(evidence.canonical_artifact_id, *canonical.id());
    assert!(evidence.lossy && evidence.review_required && evidence.candidate.review_required);
    assert_eq!(store.read_bytes(&source).unwrap(), SOURCE);
    assert_eq!(parse_json(&store.read_bytes(&canonical).unwrap()).unwrap(), evidence.candidate.screenplay);
}

#[test]
fn every_native_export_retains_full_candidate_and_real_canonical_parent() {
    let (_directory, store, source, canonical) = setup();
    let original = read_evidence(&canonical).unwrap();
    for format in [Format::Fountain, Format::Fdx, Format::FadeIn, Format::Osf] {
        let output = RecoveryExportArtifactTransform::new(format, policy()).unwrap()
            .apply(&canonical, format, &store).unwrap();
        validate_published_artifact(&output, &source, &store, &policy()).unwrap();
        let evidence = read_evidence(&output).unwrap();
        assert_eq!(output.sources(), [canonical.id().clone()]);
        assert_eq!(evidence.candidate, original.candidate);
        assert_eq!(evidence.extraction_evidence, original.extraction_evidence);
        assert_eq!(evidence.input_sha256, canonical.digest().value());
        assert!(evidence.lossy && evidence.review_required);
        assert!(evidence.export_evidence.unwrap().review_required);
    }
    assert_eq!(store.read_bytes(&source).unwrap(), SOURCE);
}

#[test]
fn publication_refuses_review_promotion_even_when_artifact_bytes_are_intact() {
    let (_directory, store, source, canonical) = setup();
    for field in ["review_required", "lossy"] {
        let changed = alter_metadata(&canonical, |value| value[field] = serde_json::json!(false));
        assert!(validate_published_artifact(&changed, &source, &store, &policy()).is_err());
    }
    let changed = alter_metadata(&canonical, |value| {
        value["candidate"]["review_required"] = serde_json::json!(false);
        value["candidate_sha256"] = serde_json::json!(digest_json(&value["candidate"]).unwrap());
    });
    assert!(validate_published_artifact(&changed, &source, &store, &policy()).is_err());
}

#[test]
fn publication_refuses_rehashed_extraction_claims_that_differ_from_source() {
    let (_directory, store, source, canonical) = setup();
    let changed = alter_metadata(&canonical, |value| {
        value["extraction_evidence"]["pages"][0]["text_sha256"] = serde_json::json!("b".repeat(64));
        value["extraction_evidence_sha256"] = serde_json::json!(digest_json(&value["extraction_evidence"]).unwrap());
    });
    assert!(validate_published_artifact(&changed, &source, &store, &policy()).is_err());
}

#[test]
fn publication_refuses_mutated_source_and_mutated_terminal_payload() {
    let (_directory, store, source, canonical) = setup();
    let source_path = store.payload_path(&source).unwrap();
    fs::write(&source_path, b"INT. LAB - DAY\n\nA light burns.\n").unwrap();
    assert!(validate_published_artifact(&canonical, &source, &store, &policy()).is_err());
    fs::write(source_path, SOURCE).unwrap();
    let output = RecoveryExportArtifactTransform::new(Format::Fdx, policy()).unwrap()
        .apply(&canonical, Format::Fdx, &store).unwrap();
    let mut changed = store.read_bytes(&output).unwrap();
    changed.push(b'\n');
    fs::write(store.payload_path(&output).unwrap(), changed).unwrap();
    assert!(validate_published_artifact(&output, &source, &store, &policy()).is_err());
}

#[test]
fn publication_refuses_native_derivative_with_original_source_as_direct_parent() {
    let (_directory, store, source, canonical) = setup();
    let output = RecoveryExportArtifactTransform::new(Format::Osf, policy()).unwrap()
        .apply(&canonical, Format::Osf, &store).unwrap();
    let detached = store.put_bytes(&store.read_bytes(&output).unwrap(),
        ArtifactDescriptor::for_format(Format::Osf, ArtifactStorageClass::Terminal)
            .with_source(source.id().clone())
            .with_metadata(SCREENPLAY_RECOVERY_ARTIFACT_METADATA_KEY,
                output.metadata()[SCREENPLAY_RECOVERY_ARTIFACT_METADATA_KEY].clone())).unwrap();
    assert!(validate_published_artifact(&detached, &source, &store, &policy()).is_err());
}

#[test]
fn policy_identity_and_complete_evidence_are_mandatory() {
    let (_directory, store, source, canonical) = setup();
    let mut changed_policy = policy();
    changed_policy.limits.max_pages -= 1;
    assert!(validate_published_artifact(&canonical, &source, &store, &changed_policy).is_err());
    let missing = store.put_bytes(&store.read_bytes(&canonical).unwrap(),
        ArtifactDescriptor::for_format(Format::ScreenplayJson, ArtifactStorageClass::Terminal)
            .with_source(source.id().clone())).unwrap();
    assert!(validate_published_artifact(&missing, &source, &store, &policy()).is_err());
}

#[test]
fn source_and_derivative_media_types_are_mandatory_without_generic_validation() {
    let (_directory, store, source, canonical) = setup();
    let mut value = serde_json::to_value(&canonical).unwrap();
    value["media_type"] = serde_json::json!("application/octet-stream");
    let changed: Artifact = serde_json::from_value(value).unwrap();
    assert!(validate_published_artifact(&changed, &source, &store, &policy()).is_err());
    let mut value = serde_json::to_value(&source).unwrap();
    value["media_type"] = serde_json::json!("text/markdown");
    let changed_source: Artifact = serde_json::from_value(value).unwrap();
    assert!(RecoveryArtifactTransform::new(policy(), None).unwrap()
        .apply(&changed_source, Format::ScreenplayJson, &store).is_err());
}

#[test]
fn recovery_refuses_cancellation_nonoriginal_input_and_direct_pdf_export() {
    let (_directory, store, source, canonical) = setup();
    let cancelled = RecoveryArtifactTransform::new(policy(), Some(Arc::new(AtomicBool::new(true)))).unwrap();
    assert!(cancelled.apply(&source, Format::ScreenplayJson, &store).unwrap_err().to_string().contains("cancelled"));
    let derived = store.put_bytes(SOURCE,
        ArtifactDescriptor::for_format(Format::Text, ArtifactStorageClass::Intermediate)
            .with_source(source.id().clone())).unwrap();
    assert!(RecoveryArtifactTransform::new(policy(), None).unwrap().apply(&derived, Format::ScreenplayJson, &store).is_err());
    assert!(RecoveryExportArtifactTransform::new(Format::Pdf, policy()).is_err());
    assert!(RecoveryArtifactTransform::new(policy(), None).unwrap().apply(&source, Format::Pdf, &store).is_err());
    assert!(RecoveryExportArtifactTransform::new(Format::Osf, policy()).unwrap().apply(&canonical, Format::Fdx, &store).is_err());
}

#[test]
fn recovery_and_exports_are_lossy_and_not_cacheable() {
    let recover = RecoveryArtifactTransform::new(policy(), None).unwrap();
    assert!(!recover.cacheable());
    assert_eq!(recover.fidelity(), Some(FidelityDeclaration::Lossy));
    let export = RecoveryExportArtifactTransform::new(Format::Fdx, policy()).unwrap();
    assert!(!export.cacheable());
    assert_eq!(export.fidelity(), Some(FidelityDeclaration::Lossy));
}

#[test]
fn unstructured_text_identity_does_not_imply_markdown() {
    assert_eq!("txt".parse::<Format>().unwrap(), Format::Text);
    assert_eq!(Format::Text.to_string(), "text");
    assert_eq!(crate::artifact::MediaType::for_format(Format::Text).as_str(), "text/plain");
    assert_eq!(crate::detect::detect_from_extension("draft.txt"), Some(Format::Text));
    assert_ne!(Format::Text, Format::Markdown);
}
