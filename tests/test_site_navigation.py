"""Protect topic discovery and the reviewed gallery source/render pairing."""

from pathlib import Path
import hashlib
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def pages(items: list[dict]) -> list[str]:
    return [
        page
        for item in items
        for value in item.values()
        for page in (pages(value) if isinstance(value, list) else [value])
    ]


def test_public_pages_have_navigation_context() -> None:
    config = tomllib.loads((ROOT / "zensical.toml").read_text())
    sections = {
        label: value
        for item in config["project"]["nav"]
        for label, value in item.items()
    }
    assert all(
        isinstance(sections[label], list)
        for label in (
            "Learn",
            "Guides",
            "Concepts",
            "Examples",
            "Reference",
            "Troubleshooting",
            "Contributing",
        )
    )
    assert pages(sections["Learn"]) == [
        "learn/installation.md",
        "learn/first-render.md",
        "learn/first-animation.md",
        "learn/animation.md",
    ]
    assert pages(sections["Contributing"])[:2] == [
        "development/README.md",
        "development/documentation.md",
    ]
    navigated = pages(config["project"]["nav"])
    public = {
        str(path.relative_to(ROOT / "docs"))
        for path in (ROOT / "docs").rglob("*.md")
        if path.relative_to(ROOT / "docs").parts[0] not in {"agents", "plans"}
    }
    assert public <= set(navigated), public - set(navigated)
    assert len(navigated) == len(set(navigated)), "Each page needs one sidebar context"


def test_gallery_links_and_setup_use_the_cleaned_checkout() -> None:
    gallery = ROOT / "docs/gallery"
    index = (gallery / "index.md").read_text()
    revision = re.search(r"git switch --detach ([0-9a-f]{40})", index)[1]
    assert "docs/readme-landing" not in index
    for path in gallery.glob("*.md"):
        if path.name == "index.md":
            continue
        text = path.read_text()
        links = re.findall(
            r"https://github.com/evgen2571/vestra/blob/([0-9a-f]{40})/"
            r"(examples/showcase/[^)#\s]+)",
            text,
        )
        assert any(name.endswith("/main.py") for _, name in links), path
        assert any(name.endswith("/render.mp4") for _, name in links), path
        for linked_revision, name in links:
            assert linked_revision == revision, path
            committed = subprocess.check_output(
                ["git", "show", f"{revision}:{name}"], cwd=ROOT
            )
            assert (ROOT / name).read_bytes() == committed, name
            if name.endswith("/render.mp4"):
                assert hashlib.sha256(committed).hexdigest() in text, path
