"""Tests for CLI command structure and option parsing."""
from click.testing import CliRunner

from capcut_cli.cli import main


runner = CliRunner()


class TestCLIStructure:
    def test_main_help(self):
        result = runner.invoke(main, ["--help"])
        assert result.exit_code == 0
        assert "Agent-first video editing CLI" in result.output

    def test_discover_group_exists(self):
        result = runner.invoke(main, ["discover", "--help"])
        assert result.exit_code == 0
        assert "tiktok-sounds" in result.output
        assert "x-clips" in result.output

    def test_library_group_exists(self):
        result = runner.invoke(main, ["library", "--help"])
        assert result.exit_code == 0
        assert "import" in result.output
        assert "list" in result.output
        assert "show" in result.output
        assert "delete" in result.output

    def test_compose_help(self):
        result = runner.invoke(main, ["compose", "--help"])
        assert result.exit_code == 0
        assert "--sound" in result.output
        assert "--clip" in result.output
        assert "--duration" in result.output
        assert "--resolution" in result.output

    def test_compose_has_loudness_option(self):
        result = runner.invoke(main, ["compose", "--help"])
        assert result.exit_code == 0
        assert "--loudness" in result.output
        assert "viral" in result.output
        assert "podcast" in result.output

    def test_deps_group_exists(self):
        result = runner.invoke(main, ["deps", "--help"])
        assert result.exit_code == 0
        assert "check" in result.output
        assert "install" in result.output
