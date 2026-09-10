use std::collections::BTreeMap;

use anyhow::Result;
use renderflow_plugin_sdk::{
    ArtifactCollection, PluginExecutionContext, PluginInputKind, PluginTransformDescriptor,
    PluginTransformRequest, PluginTransformResult, PluginTransformV2,
};

struct OrderedJoin;

impl PluginTransformV2 for OrderedJoin {
    fn descriptor(&self) -> PluginTransformDescriptor {
        PluginTransformDescriptor::new(
            "example.ordered-join",
            "1.0.0",
            "transform.collection.join",
            PluginInputKind::OrderedCollection,
        )
        .with_formats(["markdown"], ["markdown"])
    }

    fn execute(
        &self,
        request: PluginTransformRequest,
        context: &PluginExecutionContext,
    ) -> Result<PluginTransformResult> {
        let mut joined = Vec::new();
        for (index, input) in request.inputs.iter().enumerate() {
            if index > 0 {
                joined.extend_from_slice(b"\n");
            }
            joined.extend(context.artifacts.read_input_bytes(input)?);
        }
        let output = context
            .artifacts
            .commit_bytes(&joined, None, BTreeMap::new())?;
        Ok(PluginTransformResult {
            outputs: ArtifactCollection::one(output),
            ..PluginTransformResult::default()
        })
    }
}

fn main() {
    let _plugin = OrderedJoin;
}
