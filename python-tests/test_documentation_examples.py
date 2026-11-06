"""Exercise guide snippets against the public API, including cumulative edits."""

from pathlib import Path
import re
import shutil
import textwrap
from types import SimpleNamespace

import pytest
from vestra import Project

ROOT = Path(__file__).resolve().parents[1]
GUIDES = sorted((ROOT / "docs/guides/python").glob("*.md"))


def snippets(page: Path) -> list[str]:
    return [
        textwrap.dedent(block)
        for block in re.findall(
            r"^ *```python[^\n]*\n(.*?)^ *```",
            page.read_text(),
            re.MULTILINE | re.DOTALL,
        )
    ]


@pytest.mark.parametrize("page", GUIDES, ids=lambda page: page.stem)
def test_guide_snippets_author_valid_projects(page, tmp_path, monkeypatch):
    assets = tmp_path / "assets"
    assets.mkdir()
    for name in ("photo.png", "one.png", "two.png"):
        shutil.copyfile(ROOT / "examples/assets/red.png", assets / name)
    for name in ("character.png", "gradient.png", "alpha.png"):
        shutil.copyfile(ROOT / "examples/assets/green.png", tmp_path / name)
    shutil.copyfile(ROOT / "tests/assets/VestraTest-Regular.ttf", assets / "MyFont.ttf")
    shutil.copyfile(ROOT / "examples/assets/tone.wav", assets / "music.wav")
    monkeypatch.chdir(tmp_path)
    renders = []

    def capture_render(self, output, **settings):
        renders.append((output, settings))
        return SimpleNamespace(output_path=output, selected_backend="cpu")

    monkeypatch.setattr(Project, "render", capture_render)
    # The rendering guide explicitly starts with a project from the first tutorial.
    namespace = {"project": Project(size=(160, 90), fps=30, duration=3)}
    for index, code in enumerate(snippets(page)):
        exec(compile(code, f"{page.name}:snippet-{index + 1}", "exec"), namespace)
        if page.stem == "effects" and "custom_letters =" in code:
            assert len(namespace["photo"].effects.items) == 1, (
                "The custom-font alternative should show one ASCII treatment"
            )
        if page.stem == "effects" and re.search(
            r"^look(?:\s*=|\.)", code, re.MULTILINE
        ):
            assert namespace["look"] in namespace["photo"].effects.items, (
                "Palette adjustments must edit an effect attached to the displayed photo"
            )
        report = namespace["project"].validate()
        assert report.is_valid, (page.name, index + 1, report.diagnostics)

    if page.stem == "masks":
        assert renders == [
            ("portrait-reveal.mp4", {"backend": "cpu", "overwrite": True})
        ]


def test_nested_group_example_places_the_badge() -> None:
    namespace = {}
    page = ROOT / "docs/guides/python/nested-compositions.md"
    for code in snippets(page)[:3]:
        exec(code, namespace)
    clips = namespace["project"].snapshot().to_dict()["visual"]["clips"]
    card = next(
        clip["source"]
        for clip in clips
        if clip["source"].get("size") == {"width": 600, "height": 300}
    )
    assert any(
        child["source"].get("size") == {"width": 120, "height": 60}
        for child in card["clips"]
    ), "Place the card after adding its badge: placement snapshots the source"
