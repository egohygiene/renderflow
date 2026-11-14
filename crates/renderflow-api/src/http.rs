//! Deliberately bounded local HTTP/1.1 transport. httparse owns syntax parsing;
//! one Content-Length request is handled per connection, then it is closed.

mod framing;
mod router;
mod server;

use crate::{ApiError, ApiResult, ServiceConfig};
pub use router::{Authorizer, BearerToken};
use serde::{Deserialize, Serialize};
pub use server::serve;
use std::net::SocketAddr;

pub const HTTP_SCHEMA: &str = "renderflow.http-service/v1";
pub const OPENAPI: &str = include_str!("../../../docs/http-api.openapi.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpConfig {
    pub schema_version: String,
    pub bind: SocketAddr,
    #[serde(default)]
    pub http: HttpLimits,
    pub service: ServiceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpLimits {
    pub workers: usize,
    pub connections: usize,
    pub header_bytes: usize,
    pub headers: usize,
    pub body_bytes: usize,
    pub upload_bytes: usize,
    pub json_response_bytes: usize,
    pub read_idle_ms: u64,
    pub read_deadline_ms: u64,
    pub write_deadline_ms: u64,
    pub shutdown_grace_ms: u64,
}
impl Default for HttpLimits {
    fn default() -> Self {
        Self {
            workers: 4,
            connections: 8,
            header_bytes: 8192,
            headers: 32,
            body_bytes: 16384,
            upload_bytes: 8 * 1024 * 1024,
            json_response_bytes: 4 * 1024 * 1024,
            read_idle_ms: 1000,
            read_deadline_ms: 5000,
            write_deadline_ms: 5000,
            shutdown_grace_ms: 5000,
        }
    }
}
impl HttpConfig {
    pub fn validate(&self) -> ApiResult<()> {
        self.service.limits.validate()?;
        let h = &self.http;
        if self.schema_version != HTTP_SCHEMA
            || !self.bind.ip().is_loopback()
            || !(1..=16).contains(&h.workers)
            || h.connections < h.workers
            || h.connections > 64
            || !(1024..=16384).contains(&h.header_bytes)
            || !(1..=64).contains(&h.headers)
            || !(2..=65536).contains(&h.body_bytes)
            || !(1..=64 * 1024 * 1024).contains(&h.upload_bytes)
            || h.json_response_bytes < self.service.limits.result_bytes + 128 * 1024
            || h.json_response_bytes > 8 * 1024 * 1024
            || !(100..=5000).contains(&h.read_idle_ms)
            || h.read_deadline_ms < h.read_idle_ms
            || h.read_deadline_ms > 30000
            || !(100..=30000).contains(&h.write_deadline_ms)
            || h.shutdown_grace_ms > 30000
        {
            return Err(ApiError::new(
                "invalid_http_config",
                "HTTP requires loopback binding and supported finite transport limits",
                400,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
