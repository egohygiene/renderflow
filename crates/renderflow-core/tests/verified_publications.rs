//! Authored and unrun while executable qualification is deferred by the maintainer.

#[path = "common/publication_fixture.rs"]
mod fixture;

use renderflow::evidence::verification::{PinnedRunReceipt, VerificationError, VerificationLimits};
use renderflow::evidence::{RunState, ValidationState};
use renderflow::process::ProcessCancellationToken;
use std::fs;

fn token() -> ProcessCancellationToken {
    ProcessCancellationToken::new()
}

#[test]
fn streamed_observation_handles_over_32_mib_with_bounded_sink_writes() {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let mut file = fs::File::create(root.path().join("result.txt")).unwrap();
    let chunk = [b'x'; 65536];
    let mut hash = Sha256::new();
    for _ in 0..513 {
        file.write_all(&chunk).unwrap();
        hash.update(chunk);
    }
    drop(file);
    let mut run = fixture::manifest(b"");
    let size = 513 * 65536;
    run.artifact_manifest.artifacts[0].size_bytes = size;
    run.artifact_manifest.artifacts[0].digest.value = format!("{:x}", hash.finalize());
    struct Counter(u64);
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            assert!(bytes.len() <= 65536);
            self.0 += bytes.len() as u64;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Counter(0);
    let observed = renderflow::evidence::verification::copy_publication(
        &run,
        "target-one",
        root.path(),
        &VerificationLimits::default(),
        token(),
        &mut sink,
    )
    .unwrap();
    assert_eq!(observed.observation().size_bytes, size);
    assert_eq!(sink.0, size);
    assert_eq!(run.state, RunState::Partial);
    assert!(renderflow::evidence::verification::read_publication(
        &run,
        "target-one",
        root.path(),
        &VerificationLimits::default(),
        token()
    )
    .is_err());
}

#[test]
fn provisional_copy_sink_failure_or_cancellation_cannot_become_verified() {
    use std::io::Write;
    struct Closed;
    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("result.txt"), b"hello").unwrap();
    let run = fixture::manifest(b"hello");
    assert!(renderflow::evidence::verification::copy_publication(
        &run,
        "target-one",
        root.path(),
        &VerificationLimits::default(),
        token(),
        &mut Closed
    )
    .is_err());
    let cancelled = token();
    cancelled.cancel();
    let mut sink = Vec::new();
    assert!(matches!(
        renderflow::evidence::verification::copy_publication(
            &run,
            "target-one",
            root.path(),
            &VerificationLimits::default(),
            cancelled,
            &mut sink
        ),
        Err(VerificationError::Cancelled)
    ));
    assert!(sink.is_empty());
}

#[test]
fn bounded_delivery_returns_the_observed_bytes_and_preserves_native_state() {
    let root = tempfile::tempdir().unwrap();
    let payload = b"copied bytes\n";
    fs::write(root.path().join("result.txt"), payload).unwrap();
    let run = fixture::manifest(payload);
    let target = run.published_targets().unwrap()[0].branch.target_id.clone();
    let value = renderflow::evidence::verification::read_publication(
        &run,
        &target,
        root.path(),
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    assert_eq!(value.bytes, payload);
    assert_eq!(value.observation.sha256, fixture::digest(payload));
    assert_eq!(
        value.publication.artifact.validation,
        ValidationState::Skipped
    );
    assert_eq!(run.state, RunState::Partial);
    fs::write(root.path().join("result.txt"), b"changed bytes\n").unwrap();
    assert!(renderflow::evidence::verification::read_publication(
        &run,
        &target,
        root.path(),
        &VerificationLimits::default(),
        token()
    )
    .is_err());
}

#[test]
fn bounded_delivery_refuses_missing_targets_limits_and_cancelled_reads() {
    let root = tempfile::tempdir().unwrap();
    let run = fixture::manifest(b"hello");
    let target = run.published_targets().unwrap()[0].branch.target_id.clone();
    let limits = VerificationLimits {
        max_file_bytes: 4,
        ..Default::default()
    };
    assert!(matches!(
        renderflow::evidence::verification::read_publication(
            &run,
            &target,
            root.path(),
            &limits,
            token()
        ),
        Err(VerificationError::Limit { .. })
    ));
    assert!(renderflow::evidence::verification::read_publication(
        &run,
        "missing",
        root.path(),
        &VerificationLimits::default(),
        token()
    )
    .is_err());
    let cancelled = token();
    cancelled.cancel();
    assert!(matches!(
        renderflow::evidence::verification::read_publication(
            &run,
            &target,
            root.path(),
            &VerificationLimits::default(),
            cancelled
        ),
        Err(VerificationError::Cancelled)
    ));
}

#[test]
fn adapter_json_objects_refuse_nested_duplicates_arrays_and_oversized_bodies() {
    use renderflow::evidence::json::object;
    assert!(object::<serde_json::Value>(br#"{"nested":{"x":1,"x":2}}"#, 1024).is_err());
    assert!(object::<serde_json::Value>(b"[]", 1024).is_err());
    assert!(object::<serde_json::Value>(b"{}", 1).is_err());
    assert_eq!(
        object::<serde_json::Value>(br#"{"x":2}"#, 1024).unwrap()["x"],
        2
    );
}

#[test]
fn file_verification_preserves_partial_run_skipped_validation_and_review_metadata() {
    let root = tempfile::tempdir().unwrap();
    let payload = "Unchanged words — 你好\n".as_bytes();
    fs::write(root.path().join("result.txt"), payload).unwrap();
    let run = fixture::manifest(payload);
    let before = serde_json::to_vec(&run).unwrap();
    let files = run
        .verify_published_targets(root.path(), &VerificationLimits::default(), token())
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].observation().sha256, fixture::digest(payload));
    assert_eq!(
        files[0].publication().artifact.validation,
        ValidationState::Skipped
    );
    assert_eq!(
        files[0].publication().artifact.metadata["synthetic.review_required"],
        true
    );
    assert_eq!(run.state, RunState::Partial);
    assert_eq!(serde_json::to_vec(&run).unwrap(), before);
    assert_eq!(fs::read(root.path().join("result.txt")).unwrap(), payload);
}

#[test]
fn same_content_destinations_are_each_observed_and_tampering_is_not_hidden() {
    let root = tempfile::tempdir().unwrap();
    let payload = b"same words\n";
    fs::write(root.path().join("result.txt"), payload).unwrap();
    fs::write(root.path().join("second.txt"), payload).unwrap();
    let mut run = fixture::manifest(payload);
    fixture::second_target(&mut run);
    assert_eq!(
        run.verify_published_targets(root.path(), &VerificationLimits::default(), token())
            .unwrap()
            .len(),
        2
    );
    fs::write(root.path().join("second.txt"), b"fake words\n").unwrap();
    assert!(matches!(
        run.verify_published_targets(root.path(), &VerificationLimits::default(), token()),
        Err(VerificationError::Integrity { .. })
    ));
}

#[test]
fn traversal_alternate_separators_encoded_paths_and_directories_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let mut run = fixture::manifest(b"x");
    for locator in [
        "bundle:../outside",
        "bundle:/absolute",
        "bundle:a\\b",
        "bundle:C:/file",
        "bundle:%2e%2e/file",
        "bundle:a//b",
    ] {
        fixture::relocate(&mut run, locator);
        assert!(matches!(
            run.verify_published_targets(root.path(), &VerificationLimits::default(), token()),
            Err(VerificationError::Path { .. })
        ));
    }
    fs::create_dir(root.path().join("folder")).unwrap();
    fixture::relocate(&mut run, "bundle:folder");
    assert!(run
        .verify_published_targets(root.path(), &VerificationLimits::default(), token())
        .is_err());
}

#[test]
fn cancellation_counts_sizes_and_algorithms_fail_without_promoting_any_result() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("result.txt"), b"two").unwrap();
    let mut run = fixture::manifest(b"two");
    let cancelled = token();
    cancelled.cancel();
    assert!(matches!(
        run.verify_published_targets(root.path(), &VerificationLimits::default(), cancelled),
        Err(VerificationError::Cancelled)
    ));
    let limits = VerificationLimits {
        max_total_bytes: 2,
        ..VerificationLimits::default()
    };
    assert!(matches!(
        run.verify_published_targets(root.path(), &limits, token()),
        Err(VerificationError::Limit { .. })
    ));
    run.artifact_manifest.artifacts[0].digest.algorithm = "sha512".into();
    assert!(matches!(
        run.verify_published_targets(root.path(), &VerificationLimits::default(), token()),
        Err(VerificationError::UnsupportedDigest)
    ));
}

#[test]
fn pinned_receipt_uses_exact_selected_bytes_and_ignores_declared_host_output_paths() {
    let root = tempfile::tempdir().unwrap();
    let payload = b"hello\n";
    fs::write(root.path().join("result.txt"), payload).unwrap();
    let bytes = serde_json::to_vec_pretty(&fixture::manifest(payload)).unwrap();
    let path = root.path().join("run.json");
    fs::write(&path, &bytes).unwrap();
    let limits = VerificationLimits::default();
    let receipt =
        PinnedRunReceipt::read(&path, &fixture::digest(&bytes), &limits, token()).unwrap();
    let result = receipt.verify(root.path(), &limits, token()).unwrap();
    assert_eq!(result["run_state"], "partial");
    assert_eq!(result["verified_publication_count"], 1);
    assert_eq!(receipt.bytes(), bytes);
    assert!(PinnedRunReceipt::read(&path, &"a".repeat(64), &limits, token()).is_err());
}

#[test]
fn explicit_planned_receipt_has_no_observed_files_and_stays_planned() {
    let root = tempfile::tempdir().unwrap();
    let mut run = fixture::manifest(b"hello");
    run.state = RunState::Planned;
    let forest = run.artifact_forest.as_mut().unwrap();
    forest.produced_artifacts.clear();
    let target = &mut forest.execution.as_mut().unwrap().targets[0];
    target.outcome = renderflow::graph::ForestTargetOutcome::Planned;
    target.artifact_evidence_index = None;
    target.artifact_id = None;
    target.locator = None;
    assert!(run
        .verify_published_targets(root.path(), &VerificationLimits::default(), token())
        .unwrap()
        .is_empty());
    assert_eq!(run.state, RunState::Planned);
}

#[cfg(unix)]
#[test]
fn observed_symlink_leaves_and_intermediate_directories_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("data.txt"), b"x").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("data.txt"),
        root.path().join("result.txt"),
    )
    .unwrap();
    let mut run = fixture::manifest(b"x");
    assert!(run
        .verify_published_targets(root.path(), &VerificationLimits::default(), token())
        .is_err());
    std::os::unix::fs::symlink(outside.path(), root.path().join("linked")).unwrap();
    fixture::relocate(&mut run, "bundle:linked/data.txt");
    assert!(run
        .verify_published_targets(root.path(), &VerificationLimits::default(), token())
        .is_err());
}
