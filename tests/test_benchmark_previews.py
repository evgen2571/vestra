"""Keep the public workload gallery tied to the versioned benchmark definitions."""

import json
from pathlib import Path
import runpy

ROOT = Path(__file__).resolve().parents[1]


def test_catalog_covers_every_versioned_workload_once():
    script = runpy.run_path(str(ROOT / "scripts/render-benchmark-previews.py"))
    suites = json.loads((ROOT / "benchmarks/suites.json").read_text())
    expected = set(suites["scenarios"])
    for suite in suites["suites"].values():
        expected.update(suite.get("scenarios", []))
    assert set(script["CATALOG"]) == expected


def test_committed_previews_and_gallery_match_provenance():
    script = runpy.run_path(str(ROOT / "scripts/render-benchmark-previews.py"))
    script["check"]()


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
