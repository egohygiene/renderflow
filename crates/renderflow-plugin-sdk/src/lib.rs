//! Public plugin SDK boundary for Renderflow.
//!
//! This crate re-exports the stable plugin-facing contracts from `renderflow`.

pub use renderflow::transforms::plugin::{
    PluginCapabilities, PluginConfig, PluginContext, PluginExecutor, PluginInfo, PluginMetadata,
    PluginRegistry, PluginTransform,
};
