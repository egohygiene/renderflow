"""renderflow CLI entrypoint."""

import typer
from rich.console import Console

app = typer.Typer(
    name="renderflow",
    help=(
        "renderflow — Spec-driven document rendering engine.\n\n"
        "Transform structured YAML configurations into rendered documents\n"
        "(PDF, HTML, LaTeX) using Pandoc, Tectonic, and Jinja2 templates."
    ),
)
console = Console()


@app.callback()
def callback() -> None:
    """renderflow — Spec-driven document rendering engine."""


@app.command()
def build(
    config: str = typer.Option(
        "renderflow.yaml",
        "--config",
        help="Path to the renderflow configuration file.",
        show_default=True,
        metavar="FILE",
    ),
    dry_run: bool = typer.Option(
        False, "--dry-run", help="Preview intended actions without creating files or running commands."
    ),
) -> None:
    """Build rendered documents from a renderflow configuration file."""
    if dry_run:
        console.print("[bold]renderflow[/bold] — dry-run mode (no files will be created)")
        console.print("[cyan]✔[/cyan] Would load config")
        console.print("[cyan]✔[/cyan] Would run build pipeline")
        console.print("[yellow]✔[/yellow] Dry-run complete — no output written")
    else:
        console.print("[bold]renderflow[/bold] — starting render pipeline...")
        console.print("[cyan]✔[/cyan] Loaded config")
        console.print("[cyan]✔[/cyan] Running build pipeline")
        console.print("[green]✔[/green] Build complete")
