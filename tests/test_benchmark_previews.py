"""Keep the public workload gallery tied to the maintained benchmark definitions."""

import json
from pathlib import Path
import runpy

import pytest

ROOT = Path(__file__).resolve().parents[1]


def test_catalog_covers_every_maintained_workload_once():
    script = runpy.run_path(str(ROOT / "scripts/render-benchmark-previews.py"))
    suites = json.loads((ROOT / "benchmarks/suites.json").read_text())
    expected = set(suites["scenarios"])
    for suite in suites["suites"].values():
        expected.update(suite.get("scenarios", []))
    assert set(script["CATALOG"]) == expected


def test_committed_previews_and_gallery_match_provenance():
    script = runpy.run_path(str(ROOT / "scripts/render-benchmark-previews.py"))
    script["check"]()


def test_gallery_keeps_only_three_featured_videos_and_one_effect_sheet():
    script = runpy.run_path(str(ROOT / "scripts/render-benchmark-previews.py"))
    manifest = json.loads((ROOT / "docs/assets/benchmarks/previews.json").read_text())
    page = script["gallery"](manifest)
    videos = {
        name
        for name, row in manifest["workloads"].items()
        if row.get("delivery_sha256")
    }
    assert videos == {"editing", "layered", "production_edit"}
    assert all(
        (
            manifest["workloads"][name]["width"],
            manifest["workloads"][name]["height"],
            manifest["workloads"][name]["fps"],
        )
        == (1920, 1080, 60)
        for name in videos
    )
    assert page.count("<video ") == 3
    assert "effects.webp" in page
    for name in script["CATALOG"] | script["EXTERNAL"]:
        assert f"{{#{name}}}" in page or f'id="{name}"' in page, name


def test_unexpected_generated_media_is_rejected(monkeypatch, tmp_path):
    script = runpy.run_path(str(ROOT / "scripts/render-benchmark-previews.py"))
    check = script["check"]
    monkeypatch.setitem(check.__globals__, "ASSETS", tmp_path)
    (tmp_path / "obsolete.mp4").touch()
    with pytest.raises(AssertionError, match="Unexpected benchmark assets"):
        check()


def test_preview_source_hashes_ignore_checkout_line_endings(tmp_path):
    script = runpy.run_path(str(ROOT / "scripts/render-benchmark-previews.py"))
    source = tmp_path / "fixture.json"
    source.write_bytes(b'{\n  "frames": 90\n}\n')
    expected = script["source_digest"](source)
    source.write_bytes(b'{\r\n  "frames": 90\r\n}\r\n')
    assert script["source_digest"](source) == expected
    source.write_bytes(b'{\r\n  "frames": 91\r\n}\r\n')
    assert script["source_digest"](source) != expected


def test_gallery_check_uses_utf8_on_legacy_windows_locales(monkeypatch):
    script = runpy.run_path(str(ROOT / "scripts/render-benchmark-previews.py"))
    read_text = Path.read_text

    def windows_read(path, encoding=None, errors=None):
        return read_text(path, encoding=encoding or "cp1252", errors=errors)

    monkeypatch.setattr(Path, "read_text", windows_read)
    script["check"]()


def test_measurement_charts_use_centered_theme_aware_figures():
    for filename in ("index.md", "vestra-vs-moviepy.md"):
        page = (ROOT / "docs/benchmarks" / filename).read_text(encoding="utf-8")
        assert '<figure class="comparison" markdown="span">' in page
        assert "version 2" not in page
        assert "4070" not in page
    for filename in ("moviepy-times.svg", "effect-times.svg"):
        chart = (ROOT / "docs/assets/benchmarks" / filename).read_text(encoding="utf-8")
        assert "prefers-color-scheme:dark" in chart
        assert "var(--cpu)" in chart
        assert "var(--gpu)" in chart
        assert "#b35b35" not in chart
        assert "60 fps" in chart
