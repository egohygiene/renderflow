//! Private verified delivery snapshots. Core owns native joins/file observation;
//! the adapter owns transport reservations, temporary storage and delivery.
use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Default)]
pub(super) struct Pool {
    count: usize,
    bytes: u64,
}
struct Reservation {
    book: Arc<Book>,
    _workspace: Arc<Workspace>,
    bytes: u64,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if let Ok(mut pool) = self.book.downloads.lock() {
            pool.count = pool.count.saturating_sub(1);
            pool.bytes = pool.bytes.saturating_sub(self.bytes);
        }
    }
}

/// A single-use private file snapshot, verified before response admission. It
/// holds workspace/reservation leases until dropped, including after job expiry.
/// This is transport evidence, not publication, review or runtime qualification.
pub struct ArtifactDownload {
    pub size_bytes: u64,
    pub sha256: String,
    pub media_type: String,
    pub filename: String,
    file: std::fs::File,
    reservation: Reservation,
    deadline: Instant,
    consumed: bool,
}
impl ArtifactDownload {
    pub fn copy_to(&mut self, sink: &mut impl Write) -> ApiResult<()> {
        if self.consumed {
            return Err(ApiError::new(
                "download_consumed",
                "Delivery snapshots are single-use",
                409,
            ));
        }
        self.consumed = true;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| ApiError::io())?;
        let mut position = 0_u64;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 65536];
        while position < self.size_bytes {
            self.check()?;
            let amount = (self.size_bytes - position).min(buffer.len() as u64) as usize;
            let count = self
                .file
                .read(&mut buffer[..amount])
                .map_err(|_| ApiError::io())?;
            if count == 0 {
                return Err(integrity());
            }
            sink.write_all(&buffer[..count]).map_err(|_| {
                ApiError::new(
                    "download_interrupted",
                    "Artifact delivery did not complete",
                    500,
                )
            })?;
            hash.update(&buffer[..count]);
            position += count as u64;
        }
        self.check()?;
        let mut tail = [0_u8; 1];
        if self.file.read(&mut tail).map_err(|_| ApiError::io())? != 0
            || format!("{:x}", hash.finalize()) != self.sha256
        {
            return Err(integrity());
        }
        Ok(())
    }
    fn check(&self) -> ApiResult<()> {
        if self.reservation.book.shutdown.load(Ordering::Acquire) {
            return Err(ApiError::new(
                "shutting_down",
                "Artifact delivery cancelled by shutdown",
                503,
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(ApiError::new(
                "download_deadline",
                "Artifact delivery deadline exceeded",
                408,
            ));
        }
        Ok(())
    }
}
fn integrity() -> ApiError {
    ApiError::new(
        "artifact_integrity",
        "Artifact bytes differ from native publication evidence",
        422,
    )
}

impl Service {
    /// Observe into private temporary storage before returning any successful
    /// response headers. No whole-artifact memory buffer or unchecked direct
    /// source stream is exposed. Partial preparation drops its file/permit.
    pub fn prepare_download(&self, id: &str, artifact_id: &str) -> ApiResult<ArtifactDownload> {
        if !self.ready() {
            return Err(ApiError::new(
                "shutting_down",
                "The service is shutting down",
                503,
            ));
        }
        let index = artifact_id
            .strip_prefix("artifact-")
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 6
                    && value.bytes().all(|byte| byte.is_ascii_digit())
            })
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| ApiError::new("artifact_not_found", "Unknown job artifact ID", 404))?;
        let (result, workspace) = self.execution(id)?;
        let publications = result
            .run_manifest
            .published_targets()
            .map_err(|_| integrity())?;
        let publication = publications
            .get(index)
            .ok_or_else(|| ApiError::new("artifact_not_found", "Unknown job artifact ID", 404))?;
        let size = publication.artifact.size_bytes;
        if size > self.book.limits.stream_download_bytes {
            return Err(ApiError::new(
                "download_limit",
                "Artifact exceeds the streaming download limit",
                413,
            ));
        }
        let reservation = {
            let mut pool = self
                .book
                .downloads
                .lock()
                .map_err(|_| ApiError::internal())?;
            if pool.count >= self.book.limits.stream_downloads
                || pool.bytes + size > self.book.limits.all_stream_download_bytes
            {
                return Err(ApiError::new(
                    "download_capacity",
                    "Concurrent artifact snapshot capacity reached",
                    503,
                ));
            }
            let current = self.book.storage_bytes()?;
            if current.saturating_add(pool.bytes).saturating_add(size)
                > self.book.limits.workspace_storage_bytes
            {
                return Err(ApiError::new(
                    "storage_capacity",
                    "Insufficient private snapshot storage budget",
                    503,
                ));
            }
            pool.count += 1;
            pool.bytes += size;
            Reservation {
                book: self.book.clone(),
                _workspace: workspace.clone(),
                bytes: size,
            }
        };
        let mut file =
            tempfile::tempfile_in(self.book.directory.path()).map_err(|_| ApiError::io())?;
        let limits = renderflow::evidence::verification::VerificationLimits {
            max_file_bytes: self.book.limits.stream_download_bytes,
            max_total_bytes: self.book.limits.stream_download_bytes,
            timeout_seconds: self.book.limits.stream_timeout_seconds,
            ..Default::default()
        };
        let observed = renderflow::evidence::verification::copy_publication(
            &result.run_manifest,
            &publication.branch.target_id,
            &workspace.bundle(),
            &limits,
            self.book.download_cancellation.clone(),
            &mut file,
        )
        .map_err(|error| match error {
            renderflow::evidence::verification::VerificationError::Cancelled => {
                ApiError::new("shutting_down", "Artifact preparation cancelled", 503)
            }
            renderflow::evidence::verification::VerificationError::Deadline => ApiError::new(
                "download_deadline",
                "Artifact preparation deadline exceeded",
                408,
            ),
            renderflow::evidence::verification::VerificationError::Limit { .. } => {
                ApiError::new("download_limit", "Artifact observation limit exceeded", 413)
            }
            _ => integrity(),
        })?;
        file.flush().map_err(|_| ApiError::io())?;
        if file.metadata().map_err(|_| ApiError::io())?.len() != observed.observation().size_bytes {
            return Err(integrity());
        }
        if !self.ready() {
            return Err(ApiError::new(
                "shutting_down",
                "Artifact preparation cancelled by shutdown",
                503,
            ));
        }
        let extension = match publication
            .artifact
            .format
            .parse::<renderflow::composition::CompositionOutput>()
        {
            Ok(format) => format.extension().to_string(),
            Err(_) => crate::profiles::graph_download_extension(&publication.artifact.format)?,
        };
        Ok(ArtifactDownload {
            size_bytes: observed.observation().size_bytes,
            sha256: observed.observation().sha256.clone(),
            media_type: publication.artifact.media_type.clone(),
            filename: format!("artifact-{index}.{extension}"),
            file,
            reservation,
            deadline: Instant::now() + Duration::from_secs(self.book.limits.stream_timeout_seconds),
            consumed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::upload_tests::{Fixture, NEW};
    fn complete(fixture: &Fixture) -> (Service, String, ArtifactView) {
        let service = fixture.service();
        let upload = service
            .upload_source("lyrics", NEW.to_vec(), &package::digest(NEW))
            .unwrap();
        let job = service
            .submit(
                Operation::Execute,
                SubmitRequest {
                    pipeline_id: "lyrics".into(),
                    dry_run: false,
                    upload_id: Some(upload.upload_id),
                },
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if service.job(&job.job_id).unwrap().state.terminal() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(service.job(&job.job_id).unwrap().state, JobState::Complete);
        let row = service
            .artifacts(&job.job_id)
            .unwrap()
            .into_iter()
            .find(|row| row.format == "lyrics_json")
            .unwrap();
        (service, job.job_id, row)
    }
    fn published_path(service: &Service, id: &str, row: &ArtifactView) -> std::path::PathBuf {
        let (result, workspace) = service.execution(id).unwrap();
        let rows = result.run_manifest.published_targets().unwrap();
        let row = rows
            .iter()
            .find(|publication| publication.branch.target_id == row.target_id)
            .unwrap();
        workspace
            .bundle()
            .join(row.artifact.locator.strip_prefix("bundle:").unwrap())
    }

    #[test]
    fn streaming_exceeds_buffer_cap_and_keeps_verified_bytes_after_forget_or_source_change() {
        let mut fixture = Fixture::new();
        fixture.config.limits.download_bytes = 1;
        let (service, id, row) = complete(&fixture);
        assert_eq!(
            service.download(&id, &row.id).unwrap_err().code,
            "download_limit"
        );
        let mut download = service.prepare_download(&id, &row.id).unwrap();
        std::fs::write(
            published_path(&service, &id, &row),
            b"changed after private snapshot",
        )
        .unwrap();
        service.forget(&id).unwrap();
        let mut bytes = Vec::new();
        download.copy_to(&mut bytes).unwrap();
        assert_eq!(bytes.len() as u64, row.size_bytes);
        assert_eq!(package::digest(&bytes), row.sha256);
        let doc = renderflow::lyrics::parse_document(&bytes).unwrap();
        assert_eq!(doc.transcript.cues[0].text, "New manual words");
        assert_eq!(
            download.copy_to(&mut std::io::sink()).unwrap_err().code,
            "download_consumed"
        );
    }

    #[test]
    fn corrupt_native_source_is_refused_before_a_delivery_handle_and_releases_capacity() {
        let fixture = Fixture::new();
        let (service, id, row) = complete(&fixture);
        std::fs::write(published_path(&service, &id, &row), b"corrupt").unwrap();
        assert_eq!(
            service.prepare_download(&id, &row.id).err().unwrap().code,
            "artifact_integrity"
        );
        let pool = service.book.downloads.lock().unwrap();
        assert_eq!(pool.count, 0);
        assert_eq!(pool.bytes, 0);
    }

    #[test]
    fn concurrent_snapshot_count_and_byte_reservations_release_on_drop() {
        let mut fixture = Fixture::new();
        fixture.config.limits.stream_downloads = 1;
        let (service, id, row) = complete(&fixture);
        let download = service.prepare_download(&id, &row.id).unwrap();
        assert_eq!(
            service.prepare_download(&id, &row.id).err().unwrap().code,
            "download_capacity"
        );
        {
            let pool = service.book.downloads.lock().unwrap();
            assert_eq!(pool.count, 1);
            assert_eq!(pool.bytes, row.size_bytes);
        }
        drop(download);
        assert!(service.prepare_download(&id, &row.id).is_ok());
    }

    #[test]
    fn interrupted_writer_cannot_report_complete_and_does_not_leak_a_snapshot() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let fixture = Fixture::new();
        let (service, id, row) = complete(&fixture);
        let mut download = service.prepare_download(&id, &row.id).unwrap();
        assert_eq!(
            download.copy_to(&mut Closed).unwrap_err().code,
            "download_interrupted"
        );
        drop(download);
        assert_eq!(service.book.downloads.lock().unwrap().count, 0);
    }

    #[test]
    fn stream_size_ceiling_and_delivery_deadline_are_explicit_refusals() {
        let mut fixture = Fixture::new();
        fixture.config.limits.stream_download_bytes = 1;
        let (service, id, row) = complete(&fixture);
        assert_eq!(
            service.prepare_download(&id, &row.id).err().unwrap().code,
            "download_limit"
        );
        drop(service);
        fixture.config.limits.stream_download_bytes = 128 * 1024 * 1024;
        let (service, id, row) = complete(&fixture);
        let mut download = service.prepare_download(&id, &row.id).unwrap();
        download.deadline = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            download.copy_to(&mut std::io::sink()).unwrap_err().code,
            "download_deadline"
        );
    }

    #[test]
    fn shutdown_cancels_prepared_delivery_without_claiming_observed_completion() {
        let fixture = Fixture::new();
        let (service, id, row) = complete(&fixture);
        let mut download = service.prepare_download(&id, &row.id).unwrap();
        service.shutdown();
        assert_eq!(
            download.copy_to(&mut std::io::sink()).unwrap_err().code,
            "shutting_down"
        );
    }
}
