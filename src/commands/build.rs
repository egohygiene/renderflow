use anyhow::Result;
use tracing::info;

pub fn run() -> Result<()> {
    info!("Executing build command");
    println!("Running build pipeline");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_run_succeeds() {
        assert!(run().is_ok());
    }
}
