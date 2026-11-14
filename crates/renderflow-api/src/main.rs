use clap::Parser;
use renderflow_api::http::{self, BearerToken, HttpConfig};
use renderflow_api::{ApiError, ApiResult};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Parser)]
#[command(
    name = "renderflow-api",
    version,
    about = "Optional bounded local Renderflow service"
)]
struct Args {
    #[arg(long)]
    config: PathBuf,
}

fn run() -> ApiResult<()> {
    let args = Args::parse();
    let metadata = fs::symlink_metadata(&args.config).map_err(|_| {
        ApiError::new(
            "config_unreadable",
            "Could not read service configuration",
            400,
        )
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 65536 {
        return Err(ApiError::new(
            "invalid_config",
            "HTTP service configuration must be a regular JSON file within 64 KiB",
            400,
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(&args.config)
        .map_err(|_| {
            ApiError::new(
                "config_unreadable",
                "Could not open service configuration",
                400,
            )
        })?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            ApiError::new(
                "config_unreadable",
                "Could not read service configuration",
                400,
            )
        })?;
    let config: HttpConfig = renderflow::evidence::json::object(&bytes, 65536).map_err(|_| {
        ApiError::new(
            "invalid_config",
            "HTTP configuration must be a bounded JSON object with known fields and unique keys",
            400,
        )
    })?;
    config.validate()?;
    let token = std::env::var("RENDERFLOW_API_TOKEN").map_err(|_| {
        ApiError::new(
            "token_required",
            "Set RENDERFLOW_API_TOKEN before starting the local listener",
            400,
        )
    })?;
    let auth = Arc::new(BearerToken::new(token.as_bytes())?);
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Release))
        .map_err(|_| ApiError::new("signal_error", "Could not register shutdown handler", 500))?;
    let base = args
        .config
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    http::serve(config, base, auth, stop)
}
fn main() {
    if let Err(error) = run() {
        eprintln!("renderflow-api: {error}");
        std::process::exit(1);
    }
}
