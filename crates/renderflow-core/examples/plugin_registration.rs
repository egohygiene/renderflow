use std::sync::Arc;

use renderflow::transforms::plugin::{PluginExecutor, PluginMetadata, PluginRegistry};

struct UppercasePlugin;

impl PluginExecutor for UppercasePlugin {
    fn name(&self) -> &str {
        "uppercase"
    }

    fn execute(&self, input: String) -> anyhow::Result<String> {
        Ok(input.to_uppercase())
    }
}

fn main() -> anyhow::Result<()> {
    let mut registry = PluginRegistry::new();
    registry.register_with_metadata(
        Arc::new(UppercasePlugin),
        PluginMetadata::new("uppercase", "1.0.0")
            .with_author("renderflow example")
            .with_description("Uppercases input text")
            .with_supported_transforms(vec!["text->text".to_string()]),
    )?;

    let plugin = registry
        .get("uppercase")
        .ok_or_else(|| anyhow::anyhow!("plugin was not registered"))?;
    let output = plugin.execute("hello".to_string())?;
    assert_eq!(output, "HELLO");

    Ok(())
}
