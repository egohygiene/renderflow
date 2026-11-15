use super::*;
use serde_json::{json, Value};

fn refusal<T>(result: Result<T, Failure>, state: ValidationState, code: &str) {
    match result {
        Err(failure) => {
            assert_eq!(failure.state, state);
            assert_eq!(failure.code, code);
        }
        Ok(_) => panic!("expected owning native refusal {code}"),
    }
}
fn policy() -> AudioValidationPolicy {
    AudioValidationPolicy {
        profile: policy::PROFILE.to_owned(),
        ffmpeg: Tool {
            path: "/missing/native-audio-test/ffmpeg".to_owned(),
            sha256: "a".repeat(64),
        },
        ffprobe: Tool {
            path: "/missing/native-audio-test/ffprobe".to_owned(),
            sha256: "b".repeat(64),
        },
    }
}
fn probe_value(name: &str) -> Value {
    serde_json::from_str(match name {
        "flac" => include_str!("fixtures/flac24.json"),
        "aac" => include_str!("fixtures/aac24.json"),
        _ => include_str!("fixtures/mp3.json"),
    })
    .unwrap()
}
fn clock(value: &Value, format: Format) -> Result<probe::Clock, Failure> {
    probe::Probe::parse(&serde_json::to_vec(value).unwrap())?.clock(format)
}
fn wave(bits: u16, rate: u32, frames: u64) -> (Vec<u8>, WavInspection, Vec<u8>) {
    let pcm = vec![0x2a; frames as usize * 2 * usize::from(bits / 8)];
    let block = 2 * (bits / 8);
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + pcm.len() as u32).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(rate.to_le_bytes());
    bytes.extend((rate * u32::from(block)).to_le_bytes());
    bytes.extend(block.to_le_bytes());
    bytes.extend(bits.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((pcm.len() as u32).to_le_bytes());
    bytes.extend(&pcm);
    let layout = wav::inspect(&bytes, &ProcessCancellationToken::new()).unwrap();
    (bytes, layout, pcm)
}
fn small_clock(bits: u16, frames: u64) -> probe::Clock {
    let mut c = clock(&probe_value("flac"), Format::Flac).unwrap();
    c.output_precision_bits = Some(bits);
    c.presentation_samples = frames;
    c.stream_duration_samples = frames;
    c.packet_end_sample = frames as i64;
    c.packet_count = 1;
    c
}

#[test]
fn native_audio_policy_is_closed_object_only_and_declares_no_runtime_success() {
    let value = serde_json::to_value(policy()).unwrap();
    assert_eq!(
        serde_json::from_value::<AudioValidationPolicy>(value.clone()).unwrap(),
        policy()
    );
    for (key, bad, reason) in [
        (
            "profile",
            json!("all-codecs"),
            "audio.validation.policy.profile",
        ),
        (
            "ffmpeg",
            json!({"path":"relative/ffmpeg","sha256":"a".repeat(64)}),
            "audio.validation.policy.path",
        ),
        (
            "ffmpeg",
            json!({"path":"/ffmpeg","sha256":"A".repeat(64)}),
            "audio.validation.policy.sha256",
        ),
    ] {
        let mut changed = value.clone();
        changed[key] = bad;
        let error = serde_json::from_value::<AudioValidationPolicy>(changed).unwrap_err();
        assert_eq!(error.to_string(), reason);
    }
    assert_eq!(
        serde_json::from_value::<AudioValidationPolicy>(json!([]))
            .unwrap_err()
            .to_string(),
        "invalid type: sequence, expected a native audio policy object"
    );
    let mut extra = value.clone();
    extra["assume_valid"] = json!(true);
    assert_eq!(
        serde_json::from_value::<AudioValidationPolicy>(extra)
            .unwrap_err()
            .to_string(),
        "unknown field `assume_valid`, expected one of `profile`, `ffmpeg`, `ffprobe`"
    );
    let mut array = value.clone();
    array["ffprobe"] = json!(["/ffprobe", "b".repeat(64)]);
    assert_eq!(
        serde_json::from_value::<AudioValidationPolicy>(array)
            .unwrap_err()
            .to_string(),
        "invalid type: sequence, expected a native audio policy object"
    );
    assert!(crate::spec::ValidationPolicy::default().audio.is_none());
    assert!(
        serde_json::to_value(crate::spec::ValidationPolicy::default())
            .unwrap()
            .get("audio")
            .is_none()
    );
    assert_eq!(
        crate::spec::json_schema()["$defs"]["audioValidation"],
        policy_schema()
    );
    let published: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/renderflow-v2.schema.json"
    )))
    .unwrap();
    assert_eq!(published["$defs"]["audioValidation"], policy_schema());
    assert_eq!(
        published["$defs"]["validation"]["properties"]["audio"],
        crate::spec::json_schema()["$defs"]["validation"]["properties"]["audio"]
    );
}

#[test]
fn native_audio_actual_probe_fixtures_preserve_separate_codec_clocks() {
    let flac = clock(&probe_value("flac"), Format::Flac).unwrap();
    assert_eq!(flac.output_precision_bits, Some(24));
    assert_eq!(flac.presentation_samples, 132_300);
    assert_eq!(flac.packet_start_sample, 0);
    assert_eq!(flac.packet_end_sample, 132_300);
    let mp3 = clock(&probe_value("mp3"), Format::Mp3).unwrap();
    assert_eq!(mp3.skip_samples, 1105);
    assert_eq!(mp3.discard_padding, 227);
    assert_eq!(mp3.packet_end_sample, 133_632);
    assert_eq!(mp3.presentation_samples, 132_300);
    let aac = clock(&probe_value("aac"), Format::M4aAac).unwrap();
    assert_eq!(aac.packet_start_sample, -1024);
    assert_eq!(aac.skip_samples, 1024);
    assert_eq!(aac.presentation_samples, 132_300);
    assert_eq!(aac.discard_padding, 0);
}

#[test]
fn native_audio_probe_refuses_wrong_codec_precision_profile_and_interior_packet_damage() {
    refusal(
        clock(&probe_value("aac"), Format::M4aAlac),
        ValidationState::Invalid,
        "audio.native.probe.codec_or_layout",
    );
    let mut changed = probe_value("aac");
    changed["streams"][0]["profile"] = json!("HE-AAC");
    refusal(
        clock(&changed, Format::M4aAac),
        ValidationState::Unavailable,
        "audio.native.aac_profile_unsupported",
    );
    let mut shortened = probe_value("aac");
    shortened["packets"][1]["duration"] = json!(1023);
    refusal(
        clock(&shortened, Format::M4aAac),
        ValidationState::Invalid,
        "audio.native.probe.packet_codec_clock",
    );
    let mut reordered = probe_value("flac");
    reordered["packets"][1]["pts"] = json!(0);
    refusal(
        clock(&reordered, Format::Flac),
        ValidationState::Invalid,
        "audio.native.probe.packet_sequence",
    );
    let mut precision = probe_value("flac");
    precision["streams"][0]["bits_per_raw_sample"] = json!("32");
    refusal(
        clock(&precision, Format::Flac),
        ValidationState::Unavailable,
        "audio.native.precision_unsupported",
    );
    let mut fraction = probe_value("flac");
    fraction["streams"][0]["time_base"] = json!("1/48000");
    refusal(
        clock(&fraction, Format::Flac),
        ValidationState::Invalid,
        "audio.native.probe.fractional_sample_clock",
    );
    refusal(
        probe::Probe::parse(br#"{"streams":[],"streams":[],"packets":[]}"#),
        ValidationState::Invalid,
        "audio.native.probe.json",
    );
    let mut unknown = probe_value("flac");
    unknown["streams"][0]["assume_valid"] = json!(true);
    refusal(
        clock(&unknown, Format::Flac),
        ValidationState::Invalid,
        "audio.native.probe.json",
    );
}

#[test]
fn native_audio_matching_pcm_requires_every_actual_byte_frame_and_precision() {
    let (_, source, pcm) = wave(24, 44_100, 8);
    let c = small_clock(24, 8);
    let exact = compare(source, &pcm, &c, &pcm, Format::Flac).unwrap();
    assert!(exact.equal);
    assert_eq!(exact.fidelity, FidelityDeclaration::Lossless);
    let mut damaged = pcm.clone();
    damaged[5] ^= 1;
    refusal(
        compare(source, &pcm, &c, &damaged, Format::Flac),
        ValidationState::Invalid,
        "audio.native.pcm_mismatch",
    );
    refusal(
        compare(source, &pcm, &c, &pcm[..pcm.len() - 1], Format::Flac),
        ValidationState::Invalid,
        "audio.native.decoded_frame_alignment",
    );
    refusal(
        compare(source, &pcm, &c, &pcm[..pcm.len() - 6], Format::Flac),
        ValidationState::Invalid,
        "audio.native.decoded_frame_count",
    );
    let quantized = small_clock(16, 8);
    let defaults =
        crate::audio::profile::AudioProfile::FlacLevel8.resolve(crate::audio::AudioFormat::Flac);
    refusal(
        admit_layout(source, &defaults, &quantized),
        ValidationState::Invalid,
        "audio.native.profile.layout",
    );
    let cd = crate::audio::profile::AudioProfile::AlacCdQuality
        .resolve(crate::audio::AudioFormat::M4aAlac);
    admit_layout(source, &cd, &quantized).unwrap();
    let changed = compare(
        source,
        &pcm,
        &quantized,
        &vec![0; 8 * 2 * 2],
        Format::M4aAlac,
    )
    .unwrap();
    assert!(changed.changes);
    assert!(!changed.equal);
    assert_eq!(changed.fidelity, FidelityDeclaration::Lossy);
}

#[test]
fn native_audio_declared_resampling_allows_only_actual_floor_or_ceil_and_never_lossless() {
    let (_, source, pcm) = wave(24, 44_100, 147);
    let mut c = small_clock(24, 160);
    c.rate_hz = 48_000;
    let options =
        crate::audio::profile::AudioProfile::FlacBroadcast.resolve(crate::audio::AudioFormat::Flac);
    admit_layout(source, &options, &c).unwrap();
    let converted = compare(source, &pcm, &c, &vec![0; 160 * 6], Format::Flac).unwrap();
    assert_eq!(converted.fidelity, FidelityDeclaration::Lossy);
    assert!(converted.changes);
    for wrong in [159, 161] {
        c.presentation_samples = wrong;
        refusal(
            admit_layout(source, &options, &c),
            ValidationState::Invalid,
            "audio.native.presentation_mismatch",
        );
    }
    let (_, noninteger, _) = wave(24, 44_100, 8);
    for right in [8, 9] {
        c.presentation_samples = right;
        admit_layout(noninteger, &options, &c).unwrap();
    }
    c.presentation_samples = 10;
    refusal(
        admit_layout(noninteger, &options, &c),
        ValidationState::Invalid,
        "audio.native.presentation_mismatch",
    );
}

#[test]
fn native_audio_aac_decode_tail_is_exact_lc_frame_extent_and_remains_lossy() {
    let (_, source, pcm) = wave(24, 44_100, 132_300);
    let c = clock(&probe_value("aac"), Format::M4aAac).unwrap();
    let padded = vec![0; 133_120 * 6];
    let result = compare(source, &pcm, &c, &padded, Format::M4aAac).unwrap();
    assert_eq!(result.fidelity, FidelityDeclaration::Lossy);
    refusal(
        compare(
            source,
            &pcm,
            &c,
            &padded[..padded.len() - 6],
            Format::M4aAac,
        ),
        ValidationState::Invalid,
        "audio.native.decoded_frame_count",
    );
    let (_, short_source, short_pcm) = wave(16, 44_100, 2048);
    let mut aligned = c.clone();
    aligned.presentation_samples = 2048;
    aligned.output_precision_bits = None;
    let equal = compare(
        short_source,
        &short_pcm,
        &aligned,
        &short_pcm,
        Format::M4aAac,
    )
    .unwrap();
    assert!(equal.equal);
    assert_eq!(equal.fidelity, FidelityDeclaration::Lossy);
}

#[test]
fn native_audio_source_lineage_and_missing_verifier_never_launder_lossy_history() {
    use crate::artifact::{ArtifactDescriptor, ArtifactStorageClass};
    let directory = tempfile::tempdir().unwrap();
    let store = ArtifactStore::new(directory.path()).unwrap();
    let parent = store
        .put_bytes(
            b"prior lossy lineage",
            ArtifactDescriptor::for_format(Format::Mp3, ArtifactStorageClass::Intermediate),
        )
        .unwrap();
    let (bytes, _, _) = wave(16, 44_100, 8);
    let historical = store
        .put_bytes(
            &bytes,
            ArtifactDescriptor::for_format(Format::Wav, ArtifactStorageClass::Source)
                .with_source(parent.id().clone()),
        )
        .unwrap();
    refusal(
        Context::new(
            &historical,
            Format::Wav,
            &store,
            &policy(),
            ProcessCancellationToken::new(),
        ),
        ValidationState::Unavailable,
        "audio.native.source_format_unsupported",
    );
    let source = store
        .put_bytes(
            &bytes,
            ArtifactDescriptor::for_format(Format::Wav, ArtifactStorageClass::Source),
        )
        .unwrap();
    refusal(
        Context::new(
            &source,
            Format::Wav,
            &store,
            &policy(),
            ProcessCancellationToken::new(),
        ),
        ValidationState::Unavailable,
        "audio.native.verifier_missing",
    );
}

#[test]
fn native_audio_probe_argv_keeps_ffmpeg_options_out_of_ffprobe_and_mov_refs_disabled() {
    let probe = input_arguments("mov", false);
    for forbidden in ["-n", "-nostdin", "-xerror", "-filter_threads"] {
        assert!(!probe.iter().any(|arg| arg == forbidden));
    }
    assert!(probe
        .windows(2)
        .any(|pair| pair == ["-format_whitelist", "mov"]));
    assert!(probe
        .windows(2)
        .any(|pair| pair == ["-protocol_whitelist", "file"]));
    assert!(probe.windows(2).any(|pair| pair == ["-enable_drefs", "0"]));
    assert!(probe
        .windows(2)
        .any(|pair| pair == ["-use_absolute_path", "0"]));
    let decode = input_arguments("mov", true);
    assert!(decode.iter().any(|arg| arg == "-nostdin"));
    assert!(decode.iter().any(|arg| arg == "-xerror"));
}

/// Explicit local proof over retained public synthetic fixtures; never a default
/// test, dependency acquisition or current encoder execution/provenance claim.
#[test]
#[ignore = "requires explicitly selected retained synthetic audio corpus and pinned verifier directory"]
fn native_audio_actual_retained_payloads_and_mismatches() {
    use crate::artifact::{ArtifactDescriptor, ArtifactStorageClass};
    use crate::evidence::{CacheDisposition, DigestEvidence};
    let corpus = std::path::PathBuf::from(
        std::env::var("RENDERFLOW_NATIVE_AUDIO_CORPUS").expect("select retained synthetic corpus"),
    );
    let tools = std::path::PathBuf::from(
        std::env::var("RENDERFLOW_NATIVE_AUDIO_VERIFIERS").expect("select pinned verifiers"),
    );
    let policy = AudioValidationPolicy {
        profile: policy::PROFILE.to_owned(),
        ffmpeg: Tool {
            path: tools.join("ffmpeg").to_str().unwrap().to_owned(),
            sha256: "94dbc5c9d7602f044b0ec9089cea2085d0f2adc3a2b1612eeec84f8badc6949d".to_owned(),
        },
        ffprobe: Tool {
            path: tools.join("ffprobe").to_str().unwrap().to_owned(),
            sha256: "40d68a7e8fc61e8656be62b12179142124ebd61eea5ed8afefda6e2d68b97a1d".to_owned(),
        },
    };
    let directory = tempfile::tempdir().unwrap();
    let store = ArtifactStore::new(directory.path().join("store")).unwrap();
    let read = |relative: &str, expected: &str| {
        let snapshot = FileSnapshot::read(
            &corpus.join(relative),
            "retained synthetic test fixture",
            MAX_MEDIA_BYTES,
            &ProcessCancellationToken::new(),
        )
        .unwrap();
        assert_eq!(snapshot.digest(), expected);
        snapshot.bytes(&ProcessCancellationToken::new()).unwrap()
    };
    let bytes24 = read(
        "inputs/authored-24bit-44100-stereo.wav",
        "d4a6d9be2561477ba2e1d6997e827f9e7193962c1e67a366d7fcc704fe84b824",
    );
    let bytes16 = read(
        "inputs/authored-16bit-44100-stereo.wav",
        "de0831225d00540446dd26c5b09ce1e3b2a93eba24be321fc3be27a27ae3c8ab",
    );
    let source24 = store
        .put_bytes(
            &bytes24,
            ArtifactDescriptor::for_format(Format::Wav, ArtifactStorageClass::Source),
        )
        .unwrap();
    let source16 = store
        .put_bytes(
            &bytes16,
            ArtifactDescriptor::for_format(Format::Wav, ArtifactStorageClass::Source),
        )
        .unwrap();
    let context24 = Context::new(
        &source24,
        Format::Wav,
        &store,
        &policy,
        ProcessCancellationToken::new(),
    )
    .unwrap();
    let context16 = Context::new(
        &source16,
        Format::Wav,
        &store,
        &policy,
        ProcessCancellationToken::new(),
    )
    .unwrap();
    let imported = |bytes: &[u8], format: Format, source: &Artifact| {
        store
            .put_bytes(
                bytes,
                ArtifactDescriptor::for_format(format, ArtifactStorageClass::Intermediate)
                    .with_source(source.id().clone())
                    .with_metadata("renderflow.adapter", "builtin.strategy")
                    .with_metadata("renderflow.from", "wav")
                    .with_metadata("renderflow.to", format.to_string()),
            )
            .unwrap()
    };
    let step = |artifact: &Artifact, source: &Artifact, format: Format, id: &str| StepEvidence {
        step_id: id.to_owned(),
        transform: "renderflow.strategy-adapter".to_owned(),
        transform_version: "historical encoder not attested by this replay".to_owned(),
        capability: Some(format!("transform.wav.{format}")),
        provider: Some("tool.ffmpeg".to_owned()),
        input_artifacts: vec![source.id().to_string()],
        output_artifacts: vec![artifact.id().to_string()],
        configuration_digest: DigestEvidence {
            algorithm: "sha256".to_owned(),
            value: hash(id.as_bytes()),
        },
        started_at_unix_ms: 0,
        completed_at_unix_ms: 0,
        duration_ms: 0,
        state: StepState::Reused,
        cache: CacheDisposition::Hit,
        validation: ValidationState::NotRequested,
        fidelity: FidelityDeclaration::Lossy,
        skip_reason: None,
        diagnostics: Vec::new(),
    };
    let cases = [
        (
            "outputs/pcm24-delivery/authored-audio/delivery/flac.flac",
            "75d1006075ee76cf168d56aed0387d8f481554af5c16575f599088674fa4dc2e",
            Format::Flac,
            "flac_max",
            &source24,
            &context24,
            FidelityDeclaration::Lossless,
        ),
        (
            "outputs/pcm24-delivery/authored-audio/delivery/aac-m4a.m4a",
            "70f894b6ea901dcbd3902cf17661363db72af6e8e3977b747c19e1ca8445c189",
            Format::M4aAac,
            "aac_256k",
            &source24,
            &context24,
            FidelityDeclaration::Lossy,
        ),
        (
            "outputs/pcm24-delivery/authored-audio/delivery/mp3.mp3",
            "a43514c2c079b58b37e9891dcad038f3941c9a0d92080ebb7306b0795a7d428a",
            Format::Mp3,
            "hq_320k",
            &source24,
            &context24,
            FidelityDeclaration::Lossy,
        ),
        (
            "outputs/pcm16-alac/authored-audio/delivery/aac-m4a.m4a",
            "d3a0a627865fc9a3735ebd6f8acd2cc0a8856bc6df99588438c2b5532a037262",
            Format::M4aAac,
            "aac_256k",
            &source16,
            &context16,
            FidelityDeclaration::Lossy,
        ),
        (
            "outputs/pcm16-alac/authored-audio/delivery/alac.m4a",
            "b9188654149bfcb86b8381ae91012ba38a7ca0dd175faf1daf34c33e2e0b96ea",
            Format::M4aAlac,
            "alac_cd",
            &source16,
            &context16,
            FidelityDeclaration::Lossless,
        ),
    ];
    for (index, (relative, digest, format, profile, source, context, expected)) in
        cases.iter().enumerate()
    {
        let bytes = read(relative, digest);
        let artifact = imported(&bytes, *format, source);
        let id = format!("retained-native-{index}");
        let producing = step(&artifact, source, *format, &id);
        let evidence = context
            .validate(&artifact, *format, Some(profile), &id, &producing, &store)
            .unwrap();
        assert_eq!(evidence.fidelity, *expected);
        assert_eq!(evidence.validator().state, ValidationState::Valid);
        assert!(!evidence.encoder_executable_hash_attested);
        println!(
            "NATIVE_REPLAY {}",
            serde_json::to_string(&evidence).unwrap()
        );
    }
    let flac_bytes = read(cases[0].0, cases[0].1);
    let flac = imported(&flac_bytes, Format::Flac, &source24);
    let producing = step(&flac, &source24, Format::Flac, "wrong-profile");
    refusal(
        context24.validate(
            &flac,
            Format::Flac,
            Some("flac_broadcast"),
            "wrong-profile",
            &producing,
            &store,
        ),
        ValidationState::Invalid,
        "audio.native.profile.layout",
    );
    let aac_bytes = read(cases[1].0, cases[1].1);
    let wrong_codec = imported(&aac_bytes, Format::M4aAlac, &source24);
    let producing = step(&wrong_codec, &source24, Format::M4aAlac, "wrong-codec");
    refusal(
        context24.validate(
            &wrong_codec,
            Format::M4aAlac,
            Some("alac_cd"),
            "wrong-codec",
            &producing,
            &store,
        ),
        ValidationState::Invalid,
        "audio.native.probe.codec_or_layout",
    );
    let mut changed = bytes24.clone();
    let layout = wav::inspect(&changed, &ProcessCancellationToken::new()).unwrap();
    changed[layout.data_start] ^= 1;
    let changed_source = store
        .put_bytes(
            &changed,
            ArtifactDescriptor::for_format(Format::Wav, ArtifactStorageClass::Source),
        )
        .unwrap();
    let context = Context::new(
        &changed_source,
        Format::Wav,
        &store,
        &policy,
        ProcessCancellationToken::new(),
    )
    .unwrap();
    let detached = imported(&flac_bytes, Format::Flac, &changed_source);
    let producing = step(&detached, &changed_source, Format::Flac, "pcm-mismatch");
    refusal(
        context.validate(
            &detached,
            Format::Flac,
            Some("flac_max"),
            "pcm-mismatch",
            &producing,
            &store,
        ),
        ValidationState::Invalid,
        "audio.native.pcm_mismatch",
    );
    let mut upstream = step(&flac, &source24, Format::Flac, "upstream-lossy");
    upstream.input_artifacts = vec![source16.id().to_string()];
    refusal(
        context24.validate(
            &flac,
            Format::Flac,
            Some("flac_max"),
            "upstream-lossy",
            &upstream,
            &store,
        ),
        ValidationState::Unavailable,
        "audio.native.direct_route_unsupported",
    );
    let slim = std::path::PathBuf::from(
        std::env::var("RENDERFLOW_NATIVE_AUDIO_SLIM_VERIFIERS")
            .expect("select preserved slim prerequisite control"),
    );
    let slim_policy = AudioValidationPolicy {
        profile: policy::PROFILE.to_owned(),
        ffmpeg: Tool {
            path: slim.join("ffmpeg").to_str().unwrap().to_owned(),
            sha256: "098c4ba3d9d7ca067bc9bfa004a33b5b91cf0913fa9380bcda7f3975c214380b".to_owned(),
        },
        ffprobe: Tool {
            path: slim.join("ffprobe").to_str().unwrap().to_owned(),
            sha256: "31054f1d68b781089f98f594bb304cf5c9ae96dadafafdd021719ab4101a79be".to_owned(),
        },
    };
    let slim_context = Context::new(
        &source24,
        Format::Wav,
        &store,
        &slim_policy,
        ProcessCancellationToken::new(),
    )
    .unwrap();
    let missing = slim_context
        .feature_admission(Format::Flac, Some("flac_max"))
        .unwrap();
    refusal(
        missing.ensure(),
        ValidationState::Unavailable,
        "audio.native.feature_missing",
    );
    let producing = step(&flac, &source24, Format::Flac, "missing-advertised-feature");
    refusal(
        slim_context.validate(
            &flac,
            Format::Flac,
            Some("flac_max"),
            "missing-advertised-feature",
            &producing,
            &store,
        ),
        ValidationState::Unavailable,
        "audio.native.feature_missing",
    );
    println!(
        "NATIVE_UNAVAILABLE_FEATURE {}",
        serde_json::to_string(&missing).unwrap()
    );
    assert_eq!(hash(&bytes24), source24.digest().value());
    assert_eq!(hash(&bytes16), source16.digest().value());
}
