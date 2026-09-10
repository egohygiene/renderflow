use std::collections::BTreeMap;

use anyhow::Result;
use renderflow_plugin_sdk::{
    ArtifactCollection, PluginExecutionContext, PluginInputKind, PluginTransformDescriptor,
    PluginTransformRequest, PluginTransformResult, PluginTransformV2, ProcessInput,
    ProcessOutputMode, ProcessRequest,
};

struct ExternalCat;

impl PluginTransformV2 for ExternalCat {
    fn descriptor(&self) -> PluginTransformDescriptor {
        PluginTransformDescriptor::new(
            "example.external-cat",
            "1.0.0",
            "transform.text.copy",
            PluginInputKind::Single,
        )
        .with_formats(["markdown"], ["markdown"])
        .with_required_providers(["tool.cat"])
    }

    fn execute(
        &self,
        request: PluginTransformRequest,
        context: &PluginExecutionContext,
    ) -> Result<PluginTransformResult> {
        let input = request.inputs.into_one()?;
        let result = context.process.execute_checked(
            ProcessRequest::direct("cat")
                .stdin(ProcessInput::Bytes(
                    context.artifacts.read_input_bytes(&input)?,
                ))
                .stdout(ProcessOutputMode::capture(1024 * 1024))
                .stderr(ProcessOutputMode::capture(64 * 1024)),
        )?;
        let output =
            context
                .artifacts
                .commit_bytes(result.stdout().bytes(), None, BTreeMap::new())?;
        Ok(PluginTransformResult {
            outputs: ArtifactCollection::one(output),
            ..PluginTransformResult::default()
        })
    }
}

fn main() {
    let _plugin = ExternalCat;
}
