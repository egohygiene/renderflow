use std::sync::Arc;

use anyhow::{Context, Result};

use super::{Artifact, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore};
use crate::graph::Format;
use crate::transforms::Transform;

/// Experimental artifact-native transform seam introduced by the artifact kernel.
///
/// This trait is deliberately small. Issue #357 owns stabilization/versioning of
/// the Transform v2 and plugin SDK contracts. Implementations may inspect or
/// stream the input payload through [`ArtifactStore`] without converting it to
/// UTF-8.
pub trait ArtifactTransform: Send + Sync {
    /// Human-readable transform identifier.
    fn name(&self) -> &str {
        "ArtifactTransform"
    }

    /// Stable material used as part of the artifact DAG cache key.
    ///
    /// Providers with configuration that affects output should override this or
    /// be registered with an explicit identity through the executor.
    fn cache_identity(&self) -> String {
        self.name().to_string()
    }

    /// Produce one artifact in `output_format` from `input`.
    fn apply(
        &self,
        input: &Artifact,
        output_format: Format,
        store: &ArtifactStore,
    ) -> Result<Artifact>;
}

/// Compatibility adapter that runs the existing UTF-8 [`Transform`] API inside
/// the binary-safe artifact executor.
pub struct TextTransformAdapter {
    transform: Arc<dyn Transform + Send + Sync>,
    cache_identity: String,
}

impl TextTransformAdapter {
    /// Wrap a text transform using its name as cache identity.
    pub fn new(transform: Arc<dyn Transform + Send + Sync>) -> Self {
        let cache_identity = transform.name().to_string();
        Self {
            transform,
            cache_identity,
        }
    }

    /// Wrap a text transform with explicit configuration-aware cache identity.
    pub fn with_identity(
        transform: Arc<dyn Transform + Send + Sync>,
        cache_identity: impl Into<String>,
    ) -> Self {
        Self {
            transform,
            cache_identity: cache_identity.into(),
        }
    }
}

impl ArtifactTransform for TextTransformAdapter {
    fn name(&self) -> &str {
        self.transform.name()
    }

    fn cache_identity(&self) -> String {
        self.cache_identity.clone()
    }

    fn apply(
        &self,
        input: &Artifact,
        output_format: Format,
        store: &ArtifactStore,
    ) -> Result<Artifact> {
        let input_text = store.read_text(input).with_context(|| {
            format!(
                "Text transform '{}' requires UTF-8 input; register an artifact-native transform for binary payloads",
                self.transform.name()
            )
        })?;
        let output = self
            .transform
            .apply(input_text)
            .with_context(|| format!("Text transform '{}' failed", self.transform.name()))?;
        store.put_bytes(
            output.as_bytes(),
            ArtifactDescriptor::for_format(output_format, ArtifactStorageClass::Intermediate)
                .with_source(input.id().clone())
                .with_metadata("renderflow.transform", self.transform.name()),
        )
    }
}
