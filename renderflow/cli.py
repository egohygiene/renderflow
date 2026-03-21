"""renderflow CLI entrypoint."""

import typer
from rich.console import Console

app = typer.Typer(help="renderflow – Spec-driven document rendering engine.")
console = Console()


@app.callback()
def callback() -> None:
    """renderflow – Spec-driven document rendering engine."""


@app.command()
def build() -> None:
    """Build rendered output from the current project."""
    console.print("[bold green]renderflow[/bold green] build — starting render pipeline...")
    console.print("[cyan]✓[/cyan] Build command executed successfully.")
