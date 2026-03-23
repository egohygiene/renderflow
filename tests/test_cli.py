"""Tests for the renderflow CLI."""

import pytest
from typer.testing import CliRunner

from renderflow.cli import app


@pytest.fixture()
def runner() -> CliRunner:
    """Provide a fresh Typer test runner for each test."""
    return CliRunner()


def test_cli_runs_without_crashing(runner: CliRunner) -> None:
    """The CLI entrypoint should be importable and callable."""
    result = runner.invoke(app, ["--help"])
    assert result.exit_code == 0


def test_build_command_exits_successfully(runner: CliRunner) -> None:
    """'renderflow build' should exit with code 0."""
    result = runner.invoke(app, ["build"])
    assert result.exit_code == 0


def test_build_command_output_contains_expected_content(runner: CliRunner) -> None:
    """'renderflow build' output should mention the build pipeline."""
    result = runner.invoke(app, ["build"])
    assert "build" in result.output.lower()


def test_build_command_output_contains_loaded_config_message(runner: CliRunner) -> None:
    """'renderflow build' output should include a 'Loaded config' status message."""
    result = runner.invoke(app, ["build"])
    assert "Loaded config" in result.output


def test_build_command_output_contains_pipeline_message(runner: CliRunner) -> None:
    """'renderflow build' output should include a pipeline start message."""
    result = runner.invoke(app, ["build"])
    assert "render pipeline" in result.output.lower()


def test_build_command_output_contains_success_message(runner: CliRunner) -> None:
    """'renderflow build' output should include a 'Build complete' completion message."""
    result = runner.invoke(app, ["build"])
    assert "Build complete" in result.output


def test_build_dry_run_exits_successfully(runner: CliRunner) -> None:
    """'renderflow build --dry-run' should exit with code 0."""
    result = runner.invoke(app, ["build", "--dry-run"])
    assert result.exit_code == 0


def test_build_dry_run_output_contains_dry_run_message(runner: CliRunner) -> None:
    """'renderflow build --dry-run' output should mention dry-run mode."""
    result = runner.invoke(app, ["build", "--dry-run"])
    assert "dry-run" in result.output.lower()


def test_build_dry_run_output_contains_no_output_written_message(runner: CliRunner) -> None:
    """'renderflow build --dry-run' should indicate that no output was written."""
    result = runner.invoke(app, ["build", "--dry-run"])
    assert "no output written" in result.output.lower()


def test_build_dry_run_does_not_show_build_complete(runner: CliRunner) -> None:
    """'renderflow build --dry-run' should NOT print the normal 'Build complete' message."""
    result = runner.invoke(app, ["build", "--dry-run"])
    assert "Build complete" not in result.output

