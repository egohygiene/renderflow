mod root_help;
use std::io::Write;

fn main() -> anyhow::Result<()> {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if let Some(help) = renderflow::app::root_help_error_from_args(&arguments) {
        if root_help::selected(&arguments[1..]).is_none() {
            std::io::stdout()
                .lock()
                .write_all(help.to_string().as_bytes())?;
            return Ok(());
        }
        root_help::write_if_selected(&arguments[1..]);
        help.exit();
    }
    renderflow::app::run_cli_from_env()
}
