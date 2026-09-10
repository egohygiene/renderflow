//! Public plugin SDK boundary for Renderflow.
//!
//! This crate re-exports the stable plugin-facing contracts from `renderflow`.

pub use renderflow::transforms::plugin::{
    PluginCapabilities, PluginConfig, PluginContext, PluginExecutor, PluginInfo, PluginMetadata,
    PluginRegistry, PluginTransform,
};

pub use renderflow::artifact::{
    Artifact, ArtifactCollection, ArtifactId, ArtifactStore, CanonicalFormat, MediaType,
};
pub use renderflow::graph::Format;
pub use renderflow::process::{
    ProcessCancellationToken, ProcessEnvironment, ProcessError, ProcessExecutor,
    ProcessExpectedOutput, ProcessInput, ProcessNetworkPolicy, ProcessOutputMode, ProcessRequest,
    ProcessResult,
};
pub use renderflow::transforms::plugin_v2::{
    LegacyTextPluginAdapter, PluginArtifactStore, PluginCachePolicy, PluginConfigSchema,
    PluginDeterminism, PluginDiagnostic, PluginExecutionContext, PluginInputKind,
    PluginLifecycleEvent, PluginLifecycleObserver, PluginLifecyclePhase, PluginLossProfile,
    PluginProcessService, PluginProgressEvent, PluginProgressReporter, PluginRegistrationError,
    PluginRegistryV2, PluginReplacementPolicy, PluginRuntimeOptions, PluginTransformDescriptor,
    PluginTransformRequest, PluginTransformResult, PluginTransformV2, RecordingPluginObserver,
    PLUGIN_SDK_CONTRACT,
};
