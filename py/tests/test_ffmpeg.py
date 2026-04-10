"""Tests for ffmpeg wrapper — loudnorm filter construction."""
from unittest.mock import patch, MagicMock

from capcut_cli.media.ffmpeg import normalize_audio


class TestNormalizeAudio:
    @patch("capcut_cli.media.ffmpeg._run_ffmpeg")
    def test_default_params_are_viral(self, mock_ffmpeg):
        normalize_audio("in.mp3", "out.mp3")
        args = mock_ffmpeg.call_args[0][0]
        af_flag = next(a for i, a in enumerate(args) if args[i - 1] == "-af")
        assert "I=-8.0" in af_flag
        assert "TP=-1.0" in af_flag
        assert "LRA=7" in af_flag

    @patch("capcut_cli.media.ffmpeg._run_ffmpeg")
    def test_custom_lufs(self, mock_ffmpeg):
        normalize_audio("in.mp3", "out.mp3", target_lufs=-14.0, true_peak=-1.5, loudness_range=11)
        args = mock_ffmpeg.call_args[0][0]
        af_flag = next(a for i, a in enumerate(args) if args[i - 1] == "-af")
        assert "I=-14.0" in af_flag
        assert "TP=-1.5" in af_flag
        assert "LRA=11" in af_flag

    @patch("capcut_cli.media.ffmpeg._run_ffmpeg")
    def test_broadcast_lufs(self, mock_ffmpeg):
        normalize_audio("in.mp3", "out.mp3", target_lufs=-23.0, true_peak=-1.0, loudness_range=15)
        args = mock_ffmpeg.call_args[0][0]
        af_flag = next(a for i, a in enumerate(args) if args[i - 1] == "-af")
        assert "I=-23.0" in af_flag
        assert "LRA=15" in af_flag

    @patch("capcut_cli.media.ffmpeg._run_ffmpeg")
    def test_output_sample_rate_44100(self, mock_ffmpeg):
        normalize_audio("in.mp3", "out.mp3")
        args = mock_ffmpeg.call_args[0][0]
        ar_idx = args.index("-ar")
        assert args[ar_idx + 1] == "44100"
