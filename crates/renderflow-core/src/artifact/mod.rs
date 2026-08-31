//! Binary-safe artifact kernel.
//!
//! The artifact kernel separates payload storage from transform orchestration.
//! Payloads are content-addressed, file-backed, and identified by SHA-256 so
//! documents, images, audio, video, archives, and structured data can traverse
//! the same graph without a UTF-8 assumption.

mod cache;
mod model;
mod store;
mod transform;

pub use cache::compute_artifact_node_hash;
pub(crate) use cache::{load_artifact_cache, save_artifact_cache, ArtifactCache};
pub use model::{
    Artifact, ArtifactCollection, ArtifactDescriptor, ArtifactDigest, ArtifactId, ArtifactPayload,
    ArtifactStorageClass, CanonicalFormat, DigestAlgorithm, MediaType,
};
pub use store::ArtifactStore;
pub use transform::{ArtifactTransform, TextTransformAdapter};
