use anyhow::Result;

pub trait PipelineStep {
    fn execute(&self, input: String) -> Result<String>;
}
