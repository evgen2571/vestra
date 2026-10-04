"""Check the link gate against real small documentation trees."""

from pathlib import Path
import subprocess
import sys

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/check-docs.py"


def run(root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), str(root)], text=True, capture_output=True
    )


def test_missing_relative_link_fails_with_source_location(tmp_path: Path) -> None:
    (tmp_path / "README.md").write_text("# Project\n\n[Guide](docs/missing.md)\n")
    result = run(tmp_path)
    assert result.returncode == 1
    assert "README.md:3" in result.stdout
    assert "docs/missing.md" in result.stdout


def test_images_and_reference_links_are_checked(tmp_path: Path) -> None:
    (tmp_path / "README.md").write_text(
        "![Preview](preview.gif)\n[Guide][guide]\n\n[guide]: guide.md\n"
    )
    result = run(tmp_path)
    assert result.returncode == 1
    assert "preview.gif" in result.stdout
    assert "guide.md" in result.stdout


def test_valid_nested_links_encoded_paths_and_code_blocks(tmp_path: Path) -> None:
    docs = tmp_path / "docs"
    docs.mkdir()
    (tmp_path / "LICENSE").write_text("license")
    (docs / "guide name.md").write_text(
        "# Guide\n[License](../LICENSE)\n[Home](../README.md#project)\n"
    )
    (tmp_path / "README.md").write_text(
        "# Project\n[Guide](docs/guide%20name.md)\n[Docs](docs/)\n"
        "[Remote](https://example.com/no-check)\n[Mail](mailto:test@example.com)\n"
        "```md\n[Example](missing-example.md)\n```\n"
    )
    result = run(tmp_path)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "Checked" in result.stdout
