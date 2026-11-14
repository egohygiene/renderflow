mod root_help;
fn main() -> anyhow::Result<()> {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if let Some(help) = renderflow::app::root_help_error_from_args(&arguments) {
        root_help::write_if_selected(&arguments[1..]);
        help.exit();
    }
    renderflow::app::run_cli_from_env()
}
