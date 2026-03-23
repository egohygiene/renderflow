"""Tests for the renderflow CLI."""

import re

import pytest
from typer.testing import CliRunner

from renderflow.cli import app


def strip_ansi(text: str) -> str:
    """Remove ANSI escape codes from a string."""
    return re.sub(r"\x1b\[[0-9;]*m", "", text)


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


def test_root_help_contains_description(runner: CliRunner) -> None:
    """Root '--help' output should describe what renderflow does."""
    result = runner.invoke(app, ["--help"])
    assert result.exit_code == 0
    assert "rendering" in result.output.lower() or "render" in result.output.lower()


def test_root_help_lists_build_command(runner: CliRunner) -> None:
    """Root '--help' output should list the 'build' subcommand."""
    result = runner.invoke(app, ["--help"])
    assert result.exit_code == 0
    assert "build" in result.output.lower()


def test_build_help_contains_config_option(runner: CliRunner) -> None:
    """'renderflow build --help' should document the --config option."""
    result = runner.invoke(app, ["build", "--help"])
    assert result.exit_code == 0
    assert "--config" in strip_ansi(result.output)


def test_build_help_contains_dry_run_option(runner: CliRunner) -> None:
    """'renderflow build --help' should document the --dry-run option."""
    result = runner.invoke(app, ["build", "--help"])
    assert result.exit_code == 0
    assert "--dry-run" in strip_ansi(result.output)


def test_build_config_option_accepted(runner: CliRunner) -> None:
    """'renderflow build --config <file>' should be accepted without error."""
    result = runner.invoke(app, ["build", "--config", "renderflow.yaml"])
    assert result.exit_code == 0

