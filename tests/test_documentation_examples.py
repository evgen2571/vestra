"""Keep the downloadable introduction identical to its canonical showcase."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_animation_download_matches_imported_quickstart() -> None:
    assert (ROOT / "docs/assets/examples/first_animation.py").read_bytes() == (
        ROOT / "examples/showcase/quickstart/main.py"
    ).read_bytes()
