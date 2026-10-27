//! Authored coverage only; execution remains deferred by docs/work/430/HANDOFF.md.

use super::*;

const SOURCE: &[u8] = b"INT. LAB - DAY\n\nA lamp glows & flickers.\n\nALAN\n(quietly)\nWe can begin.\n\nCUT TO:\n";

fn policy() -> BodyExportPolicy { BodyExportPolicy { allow_unqualified: true } }

fn setup() -> (tempfile::TempDir, ArtifactStore, Artifact) {
    let directory = tempfile::tempdir().unwrap();
    let store = ArtifactStore::new(directory.path()).unwrap();
    let source = store.put_bytes(SOURCE,
        ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Source)).unwrap();
    (directory, store, source)
}

fn output(source: &Artifact, format: Format, store: &ArtifactStore) -> Artifact {
    FountainBodyArtifactTransform::new(format, policy(), None).unwrap()
        .apply(source, format, store).unwrap()
}

fn alter_metadata(artifact: &Artifact, alter: impl FnOnce(&mut serde_json::Value)) -> Artifact {
    let mut value = serde_json::to_value(artifact).unwrap();
    alter(&mut value["metadata"][SCREENPLAY_BODY_ARTIFACT_METADATA_KEY]);
    serde_json::from_value(value).unwrap()
}

#[test]
fn body_policy_requires_explicit_unqualified_permission_and_rejects_provider_fields() {
    assert!(serde_json::from_str::<BodyExportPolicy>("{}").is_err());
    for value in ["[true]", "[false]", "true", "null", "{\"allow_unqualified\":null}",
        "{\"allow_unqualified\":true,\"allow_unqualified\":false}"]
    {
        assert!(serde_json::from_str::<BodyExportPolicy>(value).is_err(), "{value}");
    }
    let disabled: BodyExportPolicy = serde_json::from_str("{\"allow_unqualified\":false}").unwrap();
    assert!(disabled.validate().is_err());
    assert!(FountainBodyArtifactTransform::new(Format::Fdx, disabled, None).is_err());
    for field in ["executable", "executable_sha256", "download", "timeout_seconds"] {
        let mut value = serde_json::json!({"allow_unqualified": true});
        value[field] = serde_json::json!("unexpected");
        assert!(serde_json::from_value::<BodyExportPolicy>(value).is_err());
    }
    assert!(policy().validate().is_ok());
}

#[test]
fn canonical_output_preserves_original_parsed_model_and_retains_a_real_reference() {
    let (_directory, store, source) = setup();
    let canonical = output(&source, Format::ScreenplayJson, &store);
    validate_artifact(&canonical, &source, &store, &policy()).unwrap();
    let evidence = read_evidence(&canonical).unwrap();
    let bytes = store.read_bytes(&canonical).unwrap();
    assert_eq!(parse_json(&bytes).unwrap(), parse_native(SOURCE, NativeScreenplayFormat::Fountain).unwrap());
    assert_eq!(canonical.sources(), [source.id().clone()]);
    assert_eq!(evidence.canonical_artifact.id(), canonical.id());
    assert_eq!(store.read_bytes(&evidence.canonical_artifact).unwrap(), bytes);
    assert_eq!(evidence.original_source, source);
    assert_eq!(evidence.input_sha256, source.digest().value());
    assert_eq!(evidence.output_sha256, canonical.digest().value());
    assert!(!evidence.runtime_qualified);
    assert!(evidence.review_required);
    assert_eq!(evidence.preservation, "canonical_model_from_original_fountain");
    assert_eq!(store.read_bytes(&source).unwrap(), SOURCE);
}

#[test]
fn every_native_target_is_a_direct_original_edge_with_independently_accepted_body() {
    let (_directory, store, source) = setup();
    let original = parse_native(SOURCE, NativeScreenplayFormat::Fountain).unwrap();
    for format in [Format::Fdx, Format::FadeIn, Format::Osf, Format::Trelby] {
        let derivative = output(&source, format, &store);
        validate_artifact(&derivative, &source, &store, &policy()).unwrap();
        let evidence = read_evidence(&derivative).unwrap();
        let bytes = store.read_bytes(&derivative).unwrap();
        let reparsed = parse_native(&bytes, native_format(format).unwrap()).unwrap();
        equivalent_semantics(&original, &reparsed).unwrap();
        assert_eq!(derivative.sources(), [source.id().clone()]);
        assert_ne!(derivative.sources(), [evidence.canonical_artifact.id().clone()]);
        assert_eq!(parse_json(&store.read_bytes(&evidence.canonical_artifact).unwrap()).unwrap(), original);
        assert_eq!(evidence.canonical_artifact.sources(), [source.id().clone()]);
        assert_eq!(retained_canonical(&derivative, &source, &store, &policy()).unwrap(), evidence.canonical_artifact);
        assert_eq!(evidence.input_artifact_id, *source.id());
        assert_eq!(evidence.canonical_sha256, evidence.canonical_artifact.digest().value());
        assert_eq!(evidence.qualification, "authored_unqualified");
        assert!(!evidence.runtime_qualified);
        assert!(evidence.review_required);
        assert_eq!(evidence.preservation, "six_kind_body_semantics_with_regenerated_ids");
        assert!(!derivative.metadata().contains_key("renderflow.screenplay_recovery"));
    }
    assert_eq!(store.read_bytes(&source).unwrap(), SOURCE);
}

#[test]
fn publication_rejects_promoted_qualification_and_rewritten_evidence() {
    let (_directory, store, source) = setup();
    let derivative = output(&source, Format::Fdx, &store);
    for field in ["runtime_qualified", "review_required", "qualification", "profile",
        "preservation", "limitations", "policy_sha256", "input_sha256", "canonical_sha256",
        "output_sha256", "output_format", "schema_version"]
    {
        let changed = alter_metadata(&derivative, |value| {
            value[field] = match field {
                "runtime_qualified" => serde_json::json!(true),
                "review_required" => serde_json::json!(false),
                "limitations" => serde_json::json!([]),
                _ => serde_json::json!("tampered"),
            };
        });
        assert!(validate_artifact(&changed, &source, &store, &policy()).is_err(), "{field}");
    }
    let changed = alter_metadata(&derivative, |value| {
        value["canonical_artifact"]["metadata"] = serde_json::json!({"invented": "qualification"});
    });
    assert!(validate_artifact(&changed, &source, &store, &policy()).is_err());
    let changed = alter_metadata(&derivative, |value| {
        value["canonical_artifact"]["unsupported_claim"] = serde_json::json!("silently ignored");
    });
    assert!(validate_artifact(&changed, &source, &store, &policy()).is_err());
    let canonical = read_evidence(&derivative).unwrap().canonical_artifact;
    let alias = store.root().join("copied-canonical.json");
    fs::write(alias, store.read_bytes(&canonical).unwrap()).unwrap();
    let changed = alter_metadata(&derivative, |value| {
        value["canonical_artifact"]["payload"]["relative_path"] = serde_json::json!("copied-canonical.json");
    });
    assert!(validate_artifact(&changed, &source, &store, &policy()).is_err());
}

#[test]
fn publication_rejects_missing_evidence_media_type_and_wrong_direct_parent() {
    let (_directory, store, source) = setup();
    let derivative = output(&source, Format::Osf, &store);
    let evidence = read_evidence(&derivative).unwrap();
    let bytes = store.read_bytes(&derivative).unwrap();
    let missing = store.put_bytes(&bytes,
        ArtifactDescriptor::for_format(Format::Osf, ArtifactStorageClass::Terminal)
            .with_source(source.id().clone())).unwrap();
    assert!(validate_artifact(&missing, &source, &store, &policy()).is_err());
    let wrong_parent = store.put_bytes(&bytes,
        ArtifactDescriptor::for_format(Format::Osf, ArtifactStorageClass::Terminal)
            .with_source(evidence.canonical_artifact.id().clone())
            .with_metadata(SCREENPLAY_BODY_ARTIFACT_METADATA_KEY,
                derivative.metadata()[SCREENPLAY_BODY_ARTIFACT_METADATA_KEY].clone())).unwrap();
    assert!(validate_artifact(&wrong_parent, &source, &store, &policy()).is_err());
    let mut changed = serde_json::to_value(&derivative).unwrap();
    changed["media_type"] = serde_json::json!("application/octet-stream");
    let changed: Artifact = serde_json::from_value(changed).unwrap();
    assert!(validate_artifact(&changed, &source, &store, &policy()).is_err());
    let mut changed = serde_json::to_value(&derivative).unwrap();
    changed["metadata"]["invented_approval"] = serde_json::json!(true);
    let changed: Artifact = serde_json::from_value(changed).unwrap();
    assert!(validate_artifact(&changed, &source, &store, &policy()).is_err());
}

#[test]
fn publication_rejects_rehashed_native_payload_from_another_original() {
    let (_directory, store, source) = setup();
    let correct = output(&source, Format::Fdx, &store);
    let unrelated = store.put_bytes(b"INT. STUDIO - NIGHT\n\nA different story begins.\n",
        ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Source)).unwrap();
    let unrelated_output = output(&unrelated, Format::Fdx, &store);
    let bytes = store.read_bytes(&unrelated_output).unwrap();
    let mut evidence = read_evidence(&correct).unwrap();
    evidence.output_sha256 = digest(&bytes);
    let forged = store.put_bytes(&bytes,
        ArtifactDescriptor::for_format(Format::Fdx, ArtifactStorageClass::Terminal)
            .with_source(source.id().clone())
            .with_metadata(SCREENPLAY_BODY_ARTIFACT_METADATA_KEY, serde_json::to_value(evidence).unwrap())).unwrap();
    assert!(validate_artifact(&forged, &source, &store, &policy()).is_err());
}

#[test]
fn publication_reobserves_original_canonical_and_terminal_payloads() {
    let (_directory, store, source) = setup();
    let derivative = output(&source, Format::Trelby, &store);
    let source_path = store.payload_path(&source).unwrap();
    fs::write(&source_path, b"INT. LAB - DAY\n\nDifferent source.\n").unwrap();
    assert!(validate_artifact(&derivative, &source, &store, &policy()).is_err());
    fs::write(source_path, SOURCE).unwrap();

    let evidence = read_evidence(&derivative).unwrap();
    let canonical_path = store.payload_path(&evidence.canonical_artifact).unwrap();
    let canonical = fs::read(&canonical_path).unwrap();
    fs::write(&canonical_path, b"{}").unwrap();
    assert!(validate_artifact(&derivative, &source, &store, &policy()).is_err());
    fs::write(canonical_path, canonical).unwrap();

    let path = store.payload_path(&derivative).unwrap();
    let mut changed = fs::read(&path).unwrap();
    changed.push(b'\n');
    fs::write(path, changed).unwrap();
    assert!(validate_artifact(&derivative, &source, &store, &policy()).is_err());
}

#[test]
fn original_source_only_and_frozen_output_are_enforced() {
    let (_directory, store, source) = setup();
    let canonical = output(&source, Format::ScreenplayJson, &store);
    let derived = store.put_bytes(SOURCE,
        ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Intermediate)
            .with_source(source.id().clone())).unwrap();
    let relabelled = store.put_bytes(SOURCE,
        ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Intermediate)).unwrap();
    let transform = FountainBodyArtifactTransform::new(Format::Fdx, policy(), None).unwrap();
    assert!(transform.apply(&canonical, Format::Fdx, &store).is_err());
    assert!(transform.apply(&derived, Format::Fdx, &store).is_err());
    assert!(transform.apply(&relabelled, Format::Fdx, &store).is_err());
    assert!(transform.apply(&source, Format::Osf, &store).is_err());
    for format in [Format::Fountain, Format::Pdf, Format::Text, Format::Json] {
        assert!(FountainBodyArtifactTransform::new(format, policy(), None).is_err());
    }
    let mut changed = serde_json::to_value(&source).unwrap();
    changed["media_type"] = serde_json::json!("text/plain");
    let changed: Artifact = serde_json::from_value(changed).unwrap();
    assert!(transform.apply(&changed, Format::Fdx, &store).is_err());
}

#[test]
fn unsupported_fountain_metadata_and_constructs_are_rejected_without_recovery() {
    let (_directory, store, _source) = setup();
    for bytes in [
        b"Title: Unsupported title\n\nINT. LAB - DAY\n".as_slice(),
        b"INT. LAB - DAY #1#\n".as_slice(),
        b"INT. LAB - DAY\n\n[[An editorial note]]\n".as_slice(),
        b"INT. LAB - DAY\n\n*Styled action*\n".as_slice(),
    ] {
        let source = store.put_bytes(bytes,
            ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Source)).unwrap();
        for format in [Format::ScreenplayJson, Format::Fdx, Format::FadeIn, Format::Osf, Format::Trelby] {
            assert!(FountainBodyArtifactTransform::new(format, policy(), None).unwrap()
                .apply(&source, format, &store).is_err());
        }
        assert_eq!(store.read_bytes(&source).unwrap(), bytes);
    }
}

#[test]
fn cancellation_precedes_parsing_and_routes_remain_partial_without_cache_reuse() {
    let (_directory, store, source) = setup();
    let cancellation = Arc::new(AtomicBool::new(true));
    let transform = FountainBodyArtifactTransform::new(Format::Fdx, policy(), Some(cancellation)).unwrap();
    assert!(transform.apply(&source, Format::Fdx, &store).unwrap_err().to_string().contains("screenplay_body.cancelled"));
    assert!(!transform.cacheable());
    assert_eq!(transform.fidelity(), Some(FidelityDeclaration::Partial));
    assert_ne!(transform.cache_identity(),
        FountainBodyArtifactTransform::new(Format::Osf, policy(), None).unwrap().cache_identity());
    assert_eq!(store.read_bytes(&source).unwrap(), SOURCE);
}
