"""Tests for loudness presets and compose pipeline integration."""
import pytest

from capcut_cli.config import LOUDNESS_PRESETS, DEFAULT_LOUDNESS
from capcut_cli.media.compose import resolve_loudness


class TestLoudnessPresets:
    def test_default_is_viral(self):
        assert DEFAULT_LOUDNESS == "viral"

    def test_viral_preset_is_loud(self):
        preset = LOUDNESS_PRESETS["viral"]
        assert preset["lufs"] == -8.0
        assert preset["tp"] == -1.0

    def test_podcast_preset_is_standard(self):
        preset = LOUDNESS_PRESETS["podcast"]
        assert preset["lufs"] == -14.0

    def test_broadcast_preset_is_ebu(self):
        preset = LOUDNESS_PRESETS["broadcast"]
        assert preset["lufs"] == -23.0

    def test_all_presets_have_required_keys(self):
        for name, preset in LOUDNESS_PRESETS.items():
            assert "lufs" in preset, f"{name} missing lufs"
            assert "tp" in preset, f"{name} missing tp"
            assert "lra" in preset, f"{name} missing lra"
            assert "label" in preset, f"{name} missing label"

    def test_viral_louder_than_podcast(self):
        """Higher LUFS = louder. Viral must be louder than podcast."""
        assert LOUDNESS_PRESETS["viral"]["lufs"] > LOUDNESS_PRESETS["podcast"]["lufs"]

    def test_social_between_viral_and_podcast(self):
        viral = LOUDNESS_PRESETS["viral"]["lufs"]
        social = LOUDNESS_PRESETS["social"]["lufs"]
        podcast = LOUDNESS_PRESETS["podcast"]["lufs"]
        assert viral > social > podcast


class TestResolveLoudness:
    def test_none_returns_default(self):
        result = resolve_loudness(None)
        assert result == LOUDNESS_PRESETS[DEFAULT_LOUDNESS]

    def test_named_preset(self):
        result = resolve_loudness("podcast")
        assert result["lufs"] == -14.0

    def test_all_named_presets_resolve(self):
        for name in LOUDNESS_PRESETS:
            result = resolve_loudness(name)
            assert result["lufs"] == LOUDNESS_PRESETS[name]["lufs"]

    def test_numeric_lufs_value(self):
        result = resolve_loudness("-12")
        assert result["lufs"] == -12.0
        assert "custom" in result["label"]

    def test_numeric_positive_value(self):
        result = resolve_loudness("0")
        assert result["lufs"] == 0.0

    def test_unknown_preset_raises(self):
        with pytest.raises(RuntimeError, match="Unknown loudness preset"):
            resolve_loudness("nonexistent")

    def test_unknown_preset_lists_available(self):
        with pytest.raises(RuntimeError, match="viral"):
            resolve_loudness("invalid_name")
