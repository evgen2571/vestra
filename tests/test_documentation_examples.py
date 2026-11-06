"""Keep the downloadable introduction identical to its canonical showcase."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_animation_download_matches_imported_quickstart() -> None:
    assert (ROOT / "docs/assets/examples/first_animation.py").read_bytes() == (
        ROOT / "examples/showcase/quickstart/main.py"
    ).read_bytes()


def test_python_snippets_have_valid_syntax() -> None:
    """Catch broken edits in examples even though the site only highlights them."""
    import ast
    import re
    import textwrap

    for page in (ROOT / "docs").rglob("*.md"):
        if {"agents", "plans"} & set(page.parts):
            continue
        for block in re.finditer(
            r"^ *```python[^\n]*\n(.*?)^ *```",
            page.read_text(),
            re.MULTILINE | re.DOTALL,
        ):
            code = textwrap.dedent(block[1])
            for include in re.finditer(r'^ *--8<-- "([^"]+)"$', code, re.MULTILINE):
                source = (ROOT / include[1]).read_text()
                code = code.replace(include[0], source)
            ast.parse(code, filename=str(page))


def test_learning_media_matches_maintained_sources_and_delivery_assets():
    import hashlib
    import json

    assets = ROOT / "docs/assets/renders"
    manifest = json.loads((assets / "learning-media.json").read_text())
    for name, digest in manifest["program_sha256"].items():
        program = ROOT / f"docs/assets/examples/{name}.py"
        assert hashlib.sha256(program.read_bytes()).hexdigest() == digest, name
    for name, metadata in manifest["stills"].items():
        assert metadata["backend"] == "cpu"
        assert metadata["seconds"] == 0.7
        assert (
            hashlib.sha256((assets / f"{name}.webp").read_bytes()).hexdigest()
            == metadata["web_sha256"]
        ), name
    videos = {
        name: data
        for name, data in manifest.items()
        if name not in {"program_sha256", "stills"}
    }
    assert len(videos) == 14
    for name, metadata in videos.items():
        assert metadata["backend"] == "cpu"
        assert metadata["audio"] == name.startswith("audio-")
        assert (
            hashlib.sha256((assets / f"{name}.mp4").read_bytes()).hexdigest()
            == metadata["web_sha256"]
        ), name
        assert (
            hashlib.sha256((assets / f"{name}.webp").read_bytes()).hexdigest()
            == metadata["poster_sha256"]
        ), name
