//! Shared bounded response reading; diagnostics never echo provider payloads.
use crate::ai::AiRequest;
use anyhow::{ensure, Context, Result};
use std::io::Read;

pub(super) fn agent(request: &AiRequest, endpoint: &str) -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new();
    if let Some(timeout) = request.timeout_ms {
        builder = builder.timeout(std::time::Duration::from_millis(timeout));
    }
    if request.max_response_bytes.is_some() {
        builder = builder.redirects(0);
        let local = endpoint
            .split_once("://")
            .map(|(_, authority)| authority.trim_end_matches('/'))
            .and_then(|authority| authority.parse::<std::net::SocketAddr>().ok())
            .is_some_and(|address| address.ip().is_loopback());
        if local {
            builder = builder.try_proxy_from_env(false);
        }
    }
    builder.build()
}

pub(super) fn read(response: ureq::Response, request: &AiRequest) -> Result<String> {
    read_input(response.into_reader(), request)
}
fn read_input(mut input: impl Read, request: &AiRequest) -> Result<String> {
    let maximum = request.max_response_bytes.unwrap_or(8 * 1024 * 1024);
    ensure!(
        (1..=8 * 1024 * 1024).contains(&maximum),
        "ai.transport.response_limit"
    );
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        ensure!(
            !request
                .cancellation
                .as_ref()
                .is_some_and(|token| token.is_cancelled()),
            "ai.transport.cancelled"
        );
        let count = input
            .read(&mut chunk[..(maximum + 1 - bytes.len()).min(4096)])
            .context("ai.transport.read_failed")?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        ensure!(bytes.len() <= maximum, "ai.transport.response_limit");
    }
    String::from_utf8(bytes).context("ai.transport.invalid_utf8")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_limit_is_enforced_while_reading_without_echoing_payload() {
        let request = AiRequest::new("synthetic", "unused")
            .with_transport_controls(8, crate::process::ProcessCancellationToken::new());
        assert_eq!(
            read_input(std::io::Cursor::new(b"12345678"), &request).unwrap(),
            "12345678"
        );
        let error = read_input(std::io::Cursor::new(b"private-source-text"), &request).unwrap_err();
        assert!(
            error.to_string().contains("response_limit")
                && !error.to_string().contains("private-source-text")
        );
    }
    #[test]
    fn cancelled_response_reader_never_reads_its_input() {
        struct NoRead;
        impl Read for NoRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("cancelled response was read")
            }
        }
        let token = crate::process::ProcessCancellationToken::new();
        token.cancel();
        let request = AiRequest::new("synthetic", "unused").with_transport_controls(8, token);
        assert!(read_input(NoRead, &request)
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
    }
}
