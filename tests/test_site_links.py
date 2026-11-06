"""Exercise the generated-site gate without building Vestra or Zensical."""

from pathlib import Path
import subprocess
import sys

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/check-site.py"


def run(site: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            str(site),
            "--site-url",
            "https://example.com/vestra/",
        ],
        text=True,
        capture_output=True,
    )


def test_relative_subpath_assets_and_anchors_pass(tmp_path: Path) -> None:
    guide = tmp_path / "learn"
    guide.mkdir()
    (tmp_path / "index.html").write_text(
        '<a href="learn/#first-render">Learn</a>'
        '<a href="https://other.example/missing">External</a>'
        '<a href="mailto:hello@example.com">Email</a>'
    )
    (guide / "index.html").write_text(
        '<h1 id="first-render">First render</h1>'
        '<a href="/vestra/">Home</a>'
        '<a href="https://example.com/vestra/#top">Canonical home</a>'
        '<video src="../render.mp4" poster="../a%20frame.jpg"></video>'
    )
    (tmp_path / "index.html").write_text(
        (tmp_path / "index.html").read_text() + '<span id="top"></span>'
    )
    (tmp_path / "render.mp4").touch()
    (tmp_path / "a frame.jpg").touch()
    result = run(tmp_path)
    assert result.returncode == 0, result.stdout + result.stderr


def test_missing_assets_and_cross_page_anchors_fail(tmp_path: Path) -> None:
    (tmp_path / "index.html").write_text(
        '<a href="guide.html#missing">Broken anchor</a>\n'
        '<img src="missing.png">\n'
        '<video poster="missing.jpg"></video>'
    )
    (tmp_path / "guide.html").write_text('<h1 id="guide">Guide</h1>')
    result = run(tmp_path)
    assert result.returncode == 1
    assert "index.html:1" in result.stdout
    assert "guide.html#missing" in result.stdout
    assert "missing.png" in result.stdout
    assert "missing.jpg" in result.stdout


def test_links_escaping_repository_prefix_fail(tmp_path: Path) -> None:
    (tmp_path / "index.html").write_text(
        '<a href="/learn/">Lost prefix</a><a href="../CONTRIBUTING.md">Escaped site</a>'
    )
    result = run(tmp_path)
    assert result.returncode == 1
    assert "outside /vestra/" in result.stdout
    assert "/learn/" in result.stdout
    assert "../CONTRIBUTING.md" in result.stdout


def test_empty_site_fails(tmp_path: Path) -> None:
    result = run(tmp_path)
    assert result.returncode == 1
    assert "index.html" in result.stdout
