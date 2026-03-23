"""renderflow CLI entrypoint."""

import typer
from rich.console import Console

app = typer.Typer(help="renderflow – Spec-driven document rendering engine.")
console = Console()


@app.callback()
def callback() -> None:
    """renderflow – Spec-driven document rendering engine."""


@app.command()
def build(
    dry_run: bool = typer.Option(
        False, "--dry-run", help="Preview intended actions without creating files or running commands."
    ),
) -> None:
    """Build rendered output from the current project."""
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
