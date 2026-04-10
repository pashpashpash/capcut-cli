"""Tests for Twitter/X discovery module — API search and guided fallback."""
import os
import json
import pytest
from unittest.mock import patch, MagicMock

from capcut_cli.discover.twitter import (
    _build_queries,
    _try_api_search,
    find_viral_clips,
)


class TestBuildQueries:
    def test_generates_two_search_urls(self):
        result = _build_queries("funny cats", 1000)
        assert len(result) == 2
        assert all("url" in q and "query" in q for q in result)

    def test_first_query_uses_min_faves(self):
        result = _build_queries("dance", 5000)
        assert "min_faves:5000" in result[0]["query"]
        assert "has:videos" in result[0]["query"]

    def test_second_query_uses_lower_thresholds(self):
        result = _build_queries("dance", 5000)
        assert "min_faves:2500" in result[1]["query"]
        assert "min_retweets:500" in result[1]["query"]

    def test_urls_are_properly_encoded(self):
        result = _build_queries("test query", 100)
        for q in result:
            assert "https://x.com/search?q=" in q["url"]
            assert " " not in q["url"].split("?q=")[1]


class TestTryApiSearch:
    def test_returns_none_without_bearer_token(self):
        with patch.dict(os.environ, {}, clear=True):
            os.environ.pop("TWITTER_BEARER_TOKEN", None)
            result = _try_api_search("test", 10, 1000)
            assert result is None

    @patch("capcut_cli.discover.twitter.httpx.Client")
    def test_returns_clips_with_valid_token(self, mock_client_cls):
        api_response = {
            "data": [
                {
                    "id": "123456789",
                    "text": "Check out this viral dance video",
                    "author_id": "user1",
                    "public_metrics": {
                        "like_count": 5000,
                        "retweet_count": 200,
                        "impression_count": 100000,
                    },
                    "created_at": "2025-01-01T00:00:00Z",
                }
            ],
            "includes": {
                "users": [
                    {"id": "user1", "username": "dancer42", "name": "Cool Dancer"}
                ],
                "media": [],
            },
        }

        mock_resp = MagicMock()
        mock_resp.json.return_value = api_response
        mock_resp.raise_for_status = MagicMock()

        mock_client = MagicMock()
        mock_client.get.return_value = mock_resp
        mock_client.__enter__ = MagicMock(return_value=mock_client)
        mock_client.__exit__ = MagicMock(return_value=False)
        mock_client_cls.return_value = mock_client

        with patch.dict(os.environ, {"TWITTER_BEARER_TOKEN": "test_token"}):
            result = _try_api_search("dance", 10, 1000)

        assert result is not None
        assert len(result) == 1
        assert result[0]["tweet_url"] == "https://x.com/dancer42/status/123456789"
        assert result[0]["likes"] == 5000
        assert result[0]["username"] == "dancer42"
        assert result[0]["author"] == "Cool Dancer"

    @patch("capcut_cli.discover.twitter.httpx.Client")
    def test_filters_by_min_likes(self, mock_client_cls):
        api_response = {
            "data": [
                {
                    "id": "111",
                    "text": "Low engagement",
                    "author_id": "u1",
                    "public_metrics": {"like_count": 50, "retweet_count": 2, "impression_count": 500},
                }
            ],
            "includes": {
                "users": [{"id": "u1", "username": "user1", "name": "User 1"}],
                "media": [],
            },
        }

        mock_resp = MagicMock()
        mock_resp.json.return_value = api_response
        mock_resp.raise_for_status = MagicMock()

        mock_client = MagicMock()
        mock_client.get.return_value = mock_resp
        mock_client.__enter__ = MagicMock(return_value=mock_client)
        mock_client.__exit__ = MagicMock(return_value=False)
        mock_client_cls.return_value = mock_client

        with patch.dict(os.environ, {"TWITTER_BEARER_TOKEN": "test_token"}):
            result = _try_api_search("test", 10, 1000)

        assert result is None  # All tweets below min_likes threshold

    @patch("capcut_cli.discover.twitter.httpx.Client")
    def test_returns_none_on_http_error(self, mock_client_cls):
        import httpx
        mock_client = MagicMock()
        mock_client.get.side_effect = httpx.ConnectError("API error")
        mock_client.__enter__ = MagicMock(return_value=mock_client)
        mock_client.__exit__ = MagicMock(return_value=False)
        mock_client_cls.return_value = mock_client

        with patch.dict(os.environ, {"TWITTER_BEARER_TOKEN": "test_token"}):
            result = _try_api_search("test", 10, 1000)

        assert result is None


class TestFindViralClips:
    @patch("capcut_cli.discover.twitter._try_api_search")
    def test_returns_api_results_when_available(self, mock_api):
        mock_api.return_value = [
            {
                "tweet_url": "https://x.com/user/status/123",
                "text": "Viral clip",
                "author": "User",
                "username": "user",
                "likes": 5000,
                "retweets": 100,
                "views": 50000,
                "created_at": "2025-01-01T00:00:00Z",
            }
        ]
        result = find_viral_clips("test", limit=10, min_likes=1000)
        assert result["method"] == "api_search"
        assert result["total_found"] == 1
        assert len(result["clips"]) == 1

    @patch("capcut_cli.discover.twitter._try_api_search")
    def test_falls_back_to_guided_discovery(self, mock_api):
        mock_api.return_value = None
        result = find_viral_clips("test", limit=10, min_likes=1000)
        assert result["method"] == "guided_discovery"
        assert "search_urls" in result
        assert "instructions" in result
        assert "setup_hint" in result

    @patch("capcut_cli.discover.twitter._try_api_search")
    def test_guided_includes_import_hint(self, mock_api):
        mock_api.return_value = None
        result = find_viral_clips("dance", limit=5, min_likes=500)
        assert "capcut-cli library import" in result["import_hint"]

    @patch("capcut_cli.discover.twitter._try_api_search")
    def test_search_urls_always_present(self, mock_api):
        """Search URLs should be present in both API and guided responses."""
        mock_api.return_value = [
            {"tweet_url": "x", "text": "t", "author": "a", "username": "u",
             "likes": 5000, "retweets": 10, "views": 100, "created_at": ""}
        ]
        result = find_viral_clips("test")
        assert "search_urls" in result  # Even in api_search mode
