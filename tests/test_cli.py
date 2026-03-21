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
