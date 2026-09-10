use std::collections::BTreeMap;

use anyhow::Result;
use renderflow_plugin_sdk::{
    ArtifactCollection, PluginExecutionContext, PluginInputKind, PluginTransformDescriptor,
    PluginTransformRequest, PluginTransformResult, PluginTransformV2,
};

struct BinaryCopy;

impl PluginTransformV2 for BinaryCopy {
    fn descriptor(&self) -> PluginTransformDescriptor {
        PluginTransformDescriptor::new(
            "example.binary-copy",
            "1.0.0",
            "transform.binary.copy",
            PluginInputKind::Single,
        )
        .with_formats(["png"], ["png"])
    }

    fn execute(
        &self,
        request: PluginTransformRequest,
        context: &PluginExecutionContext,
    ) -> Result<PluginTransformResult> {
        let input = request.inputs.into_one()?;
        let bytes = context.artifacts.read_input_bytes(&input)?;
        let output = context
            .artifacts
            .commit_bytes(&bytes, None, BTreeMap::new())?;
        Ok(PluginTransformResult {
            outputs: ArtifactCollection::one(output),
            ..PluginTransformResult::default()
        })
    }
}

fn main() {
    let _plugin = BinaryCopy;
}
