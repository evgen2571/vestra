"""Protect topic discovery and the reviewed gallery source/render pairing."""

from pathlib import Path
import hashlib
import re
import subprocess
import tomllib
from urllib.parse import urljoin

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize("page", ["", "vestra-vs-moviepy"])
def test_benchmark_measurements_render_as_tables(page: str) -> None:
    generated = ROOT / "site/benchmarks" / page / "index.html"
    if not generated.is_file():
        pytest.skip("requires just docs-build; included in just site-check")
    html = generated.read_text(encoding="utf-8")
    assert "<th>Timeline</th>" in html
    assert "<td>Basic export</td>" in html
    assert "<p>| Timeline" not in html


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
        label: value for item in config["project"]["nav"] for label, value in item.items()
    }
    assert pages(sections["Get started"]) == [
        "getting-started/installation.md",
        "getting-started/first-render.md",
        "getting-started/first-animation.md",
        "getting-started/animation.md",
        "getting-started/cli-installation.md",
        "getting-started/cli-quickstart.md",
    ]
    assert {"Concepts", "Troubleshooting"} <= {
        label for item in sections["Guides"] for label in item
    }
    assert [next(iter(item)) for item in sections["Project"]] == ["Benchmarks", "Contributing"]
    navigated = pages(config["project"]["nav"])
    public = {
        str(path.relative_to(ROOT / "docs"))
        for path in (ROOT / "docs").rglob("*.md")
        if path.relative_to(ROOT / "docs").parts[0] not in {"agents", "plans"}
    }
    assert public <= set(navigated), public - set(navigated)
    assert len(navigated) == len(set(navigated)), "Each page needs one sidebar context"


def test_home_learning_path_reaches_the_first_render() -> None:
    generated = ROOT / "site/index.html"
    if not generated.is_file():
        pytest.skip("requires just docs-build; included in just site-check")
    learning_path = re.search(
        r'<ol class="learning-path">(.*?)</ol>',
        generated.read_text(encoding="utf-8"),
        re.DOTALL,
    )
    assert learning_path is not None
    destinations = re.findall(r'<h3[^>]*><a href="([^"]+)"', learning_path[1])
    assert destinations == [
        "getting-started/installation/",
        "getting-started/first-render/",
        "getting-started/first-animation/",
    ]


def test_home_hero_shows_the_vestra_reel_with_playback_on_request() -> None:
    generated = ROOT / "site/index.html"
    if not generated.is_file():
        pytest.skip("requires just docs-build; included in just site-check")
    hero = re.search(
        r'<figure class="home-hero__preview">\s*<video\b([^>]*)>(.*?)</video>',
        generated.read_text(encoding="utf-8"),
        re.DOTALL,
    )
    assert hero is not None
    for attribute in ("controls", "playsinline"):
        assert re.search(rf"\s{attribute}(?=\s|=|$)", hero[1]), attribute
    assert "autoplay" not in hero[1]
    assert 'poster="assets/hero/poster.webp"' in hero[1]
    assert 'src="assets/hero/reel.mp4"' in hero[2]


@pytest.mark.parametrize("path", ["", "getting-started/first-render/", "development/"])
def test_site_has_four_primary_tabs_and_secondary_project_links(path: str) -> None:
    generated = ROOT / "site" / path / "index.html"
    if not generated.is_file():
        pytest.skip("requires just docs-build; included in just site-check")
    html = generated.read_text(encoding="utf-8")
    tabs = re.findall(r'class="md-tabs__link">\s*([^<]+)</a>', html)
    assert [tab.strip() for tab in tabs] == ["Get started", "Guides", "Examples", "Reference"]
    root = "https://example.com/vestra/"
    brand = re.search(r'class="site-brand" href="([^"]*)">Vestra</a>', html)[1]
    version = re.search(r'class="site-version" href="([^"]*)">Dev docs</a>', html)[1]
    assert urljoin(root + path, brand) == root
    assert urljoin(root + path, version) == root + "#documentation-version"
    assert "Development documentation ·" not in html
    footer = re.search(r"<footer\b.*?</footer>", html, re.DOTALL)[0]
    for label, target in [("Benchmarks", "benchmarks/"), ("Contributing", "development/")]:
        href = re.search(rf'href="([^"]*)">{label}</a>', footer)[1]
        assert urljoin(root + path, href) == root + target


def test_reference_catalog_links_every_reference_topic() -> None:
    reference = ROOT / "docs/reference"
    topics = {
        str(path.relative_to(reference))
        for path in reference.rglob("*.md")
        if path.name not in {"index.md", "catalog.md"}
    }
    linked = set(
        re.findall(
            r"\]\(([^)#]+\.md)(?:#[^)]*)?\)", (reference / "catalog.md").read_text()
        )
    )
    assert topics <= linked, topics - linked


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
