use std::collections::BTreeMap;

use anyhow::Result;
use renderflow_plugin_sdk::{
    ArtifactCollection, PluginExecutionContext, PluginInputKind, PluginTransformDescriptor,
    PluginTransformRequest, PluginTransformResult, PluginTransformV2,
};

struct UppercaseText;

impl PluginTransformV2 for UppercaseText {
    fn descriptor(&self) -> PluginTransformDescriptor {
        PluginTransformDescriptor::new(
            "example.uppercase",
            "1.0.0",
            "transform.text.uppercase",
            PluginInputKind::Single,
        )
        .with_formats(["markdown"], ["markdown"])
    }

    fn execute(
        &self,
        request: PluginTransformRequest,
        context: &PluginExecutionContext,
    ) -> Result<PluginTransformResult> {
        let input = request.inputs.into_one()?;
        let text = String::from_utf8(context.artifacts.read_input_bytes(&input)?)?;
        let output = context.artifacts.commit_bytes(
            text.to_uppercase().as_bytes(),
            None,
            BTreeMap::new(),
        )?;
        Ok(PluginTransformResult {
            outputs: ArtifactCollection::one(output),
            ..PluginTransformResult::default()
        })
    }
}

fn main() {
    let _plugin = UppercaseText;
}
