use anyhow::Result;

pub trait Transform {
    fn apply(&self, input: String) -> Result<String>;
}
