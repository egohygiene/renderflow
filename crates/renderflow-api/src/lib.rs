//! Optional local service adapter; planning/rendering remain in the public SDK.
//! Authored, unqualified #420 checkpoint. No listener is started by this crate.

pub mod http;
pub mod model;
mod package;
mod profiles;
mod service;
mod uploads;

pub use model::*;
pub use service::{ArtifactDownload, Service};
