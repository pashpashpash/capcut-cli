"""Tests for TikTok discovery module — extraction strategies and normalization."""
import json
import pytest
from unittest.mock import patch, MagicMock
from bs4 import BeautifulSoup

from capcut_cli.discover.tiktok import (
    _normalize_sound,
    _extract_next_data,
    _extract_script_scan,
    _extract_regex,
    _try_api,
    _try_html,
    find_trending_sounds,
)


# ── _normalize_sound ─────────────────────────────────────────────────

class TestNormalizeSound:
    def test_standard_fields(self):
        raw = {
            "rank": 3,
            "title": "Original Sound",
            "author": "artist_a",
            "link": "https://tiktok.com/music/123",
            "cover": "https://img.tiktok.com/cover.jpg",
            "duration": 30,
            "promoted": True,
        }
        result = _normalize_sound(raw, rank=1)
        assert result["rank"] == 3
        assert result["title"] == "Original Sound"
        assert result["artist"] == "artist_a"
        assert result["tiktok_url"] == "https://tiktok.com/music/123"
        assert result["cover_url"] == "https://img.tiktok.com/cover.jpg"
        assert result["duration_seconds"] == 30
        assert result["is_promoted"] is True

    def test_alternate_field_names(self):
        raw = {
            "musicName": "Alt Title",
            "artistName": "alt_artist",
            "playUrl": "https://play.url",
            "coverUrl": "https://cover.url",
        }
        result = _normalize_sound(raw, rank=5)
        assert result["rank"] == 5
        assert result["title"] == "Alt Title"
        assert result["tiktok_url"] == "https://play.url"
        assert result["cover_url"] == "https://cover.url"

    def test_missing_fields_use_defaults(self):
        result = _normalize_sound({}, rank=7)
        assert result["rank"] == 7
        assert result["title"] == "Unknown"
        assert result["tiktok_url"] == ""
        assert result["duration_seconds"] == 0
        assert result["is_promoted"] is False


# ── HTML extraction strategies ───────────────────────────────────────

SAMPLE_SOUNDS = [
    {"title": "Sound A", "author": "Artist A", "rank": 1},
    {"title": "Sound B", "author": "Artist B", "rank": 2},
]


class TestExtractNextData:
    def test_extracts_from_next_data_tag(self):
        payload = json.dumps({
            "props": {"pageProps": {"data": {"soundList": SAMPLE_SOUNDS}}}
        })
        html = f'<html><head><script id="__NEXT_DATA__">{payload}</script></head></html>'
        soup = BeautifulSoup(html, "html.parser")
        result = _extract_next_data(soup)
        assert result == SAMPLE_SOUNDS

    def test_returns_none_when_no_next_data(self):
        soup = BeautifulSoup("<html><body></body></html>", "html.parser")
        assert _extract_next_data(soup) is None

    def test_returns_none_on_malformed_json(self):
        html = '<html><head><script id="__NEXT_DATA__">{not valid json</script></head></html>'
        soup = BeautifulSoup(html, "html.parser")
        assert _extract_next_data(soup) is None


class TestExtractScriptScan:
    def test_finds_soundlist_in_inline_script(self):
        payload = json.dumps({
            "props": {"pageProps": {"data": {"soundList": SAMPLE_SOUNDS}}}
        })
        html = f"<html><body><script>{payload}</script></body></html>"
        soup = BeautifulSoup(html, "html.parser")
        result = _extract_script_scan(soup)
        assert result == SAMPLE_SOUNDS

    def test_finds_alternate_nesting_path(self):
        payload = json.dumps({"data": {"soundList": SAMPLE_SOUNDS}})
        html = f"<html><body><script>{payload}</script></body></html>"
        soup = BeautifulSoup(html, "html.parser")
        result = _extract_script_scan(soup)
        assert result == SAMPLE_SOUNDS

    def test_finds_snake_case_key(self):
        payload = json.dumps({"data": {"sound_list": SAMPLE_SOUNDS}})
        html = f"<html><body><script>{payload}</script></body></html>"
        soup = BeautifulSoup(html, "html.parser")
        result = _extract_script_scan(soup)
        assert result == SAMPLE_SOUNDS

    def test_finds_flat_soundlist(self):
        payload = json.dumps({"soundList": SAMPLE_SOUNDS})
        html = f"<html><body><script>{payload}</script></body></html>"
        soup = BeautifulSoup(html, "html.parser")
        result = _extract_script_scan(soup)
        assert result == SAMPLE_SOUNDS

    def test_returns_none_when_no_matching_scripts(self):
        html = "<html><body><script>var x = 1;</script></body></html>"
        soup = BeautifulSoup(html, "html.parser")
        assert _extract_script_scan(soup) is None


class TestExtractRegex:
    def test_extracts_soundlist_via_regex(self):
        html = '{"soundList": [{"title": "Test"}], "other": 1}'
        result = _extract_regex(html)
        assert result == [{"title": "Test"}]

    def test_extracts_snake_case_via_regex(self):
        html = '{"sound_list": [{"title": "Test2"}], "x": true}'
        result = _extract_regex(html)
        assert result == [{"title": "Test2"}]

    def test_returns_none_for_no_match(self):
        assert _extract_regex("<html>nothing here</html>") is None


# ── API attempt ──────────────────────────────────────────────────────

class TestTryApi:
    @patch("capcut_cli.discover.tiktok.httpx.Client")
    def test_returns_sound_list_on_success(self, mock_client_cls):
        mock_resp = MagicMock()
        mock_resp.json.return_value = {"data": {"sound_list": SAMPLE_SOUNDS}}
        mock_resp.raise_for_status = MagicMock()

        mock_client = MagicMock()
        mock_client.get.return_value = mock_resp
        mock_client.__enter__ = MagicMock(return_value=mock_client)
        mock_client.__exit__ = MagicMock(return_value=False)
        mock_client_cls.return_value = mock_client

        result = _try_api(10, "US")
        assert result == SAMPLE_SOUNDS

    @patch("capcut_cli.discover.tiktok.httpx.Client")
    def test_returns_none_on_http_error(self, mock_client_cls):
        import httpx
        mock_client = MagicMock()
        mock_client.get.side_effect = httpx.ConnectError("connection refused")
        mock_client.__enter__ = MagicMock(return_value=mock_client)
        mock_client.__exit__ = MagicMock(return_value=False)
        mock_client_cls.return_value = mock_client

        result = _try_api(10, "US")
        assert result is None


# ── find_trending_sounds (integration) ───────────────────────────────

class TestFindTrendingSounds:
    @patch("capcut_cli.discover.tiktok._try_api")
    def test_returns_sounds_from_api(self, mock_api):
        mock_api.return_value = SAMPLE_SOUNDS
        result = find_trending_sounds(limit=10, region="US")
        assert result["source"] == "tiktok_creative_center"
        assert result["total_found"] == 2
        assert result["sounds"][0]["title"] == "Sound A"
        assert result["sounds"][1]["title"] == "Sound B"

    @patch("capcut_cli.discover.tiktok._try_html")
    @patch("capcut_cli.discover.tiktok._try_api")
    def test_falls_back_to_html(self, mock_api, mock_html):
        mock_api.return_value = None
        mock_html.return_value = SAMPLE_SOUNDS
        result = find_trending_sounds(limit=10, region="US")
        assert result["total_found"] == 2
        mock_html.assert_called_once()

    @patch("capcut_cli.discover.tiktok._try_html")
    @patch("capcut_cli.discover.tiktok._try_api")
    def test_raises_when_all_strategies_fail(self, mock_api, mock_html):
        mock_api.return_value = None
        mock_html.return_value = None
        with pytest.raises(RuntimeError, match="Both the JSON API and HTML extraction failed"):
            find_trending_sounds()

    @patch("capcut_cli.discover.tiktok._try_api")
    def test_respects_limit(self, mock_api):
        many_sounds = [{"title": f"S{i}", "rank": i} for i in range(20)]
        mock_api.return_value = many_sounds
        result = find_trending_sounds(limit=5)
        assert result["total_found"] == 5
        assert len(result["sounds"]) == 5
