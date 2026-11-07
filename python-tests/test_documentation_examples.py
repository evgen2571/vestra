"""Exercise guide snippets against the public API, including cumulative edits."""

from pathlib import Path
import re
import runpy
import shutil
import textwrap
from types import SimpleNamespace

import pytest
from vestra import Project

ROOT = Path(__file__).resolve().parents[1]
GUIDES = sorted((ROOT / "docs/guides/python").glob("*.md"))


def snippets(page: Path) -> list[str]:
    blocks = [
        textwrap.dedent(block)
        for block in re.findall(
            r"^ *```python[^\n]*\n(.*?)^ *```",
            page.read_text(),
            re.MULTILINE | re.DOTALL,
        )
    ]
    return [
        re.sub(
            r'^ *--8<-- "([^"]+)"$',
            lambda match: (ROOT / match[1]).read_text(),
            code,
            flags=re.MULTILINE,
        )
        for code in blocks
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
    if page.stem == "effect-catalog":
        # Catalog blocks show constructor settings; setup lives in the linked source.
        namespace.update(
            runpy.run_path(str(ROOT / "docs/assets/examples/effects_catalog.py"))
        )
    for index, code in enumerate(snippets(page)):
        exec(compile(code, f"{page.name}:snippet-{index + 1}", "exec"), namespace)
        if page.stem == "effects":
            assert namespace["blur"] in namespace["title"].effects.items
        if page.stem == "ascii":
            assert namespace["letters"] in namespace["photo"].effects.items
        if page.stem == "palettes-and-dithering" and re.search(
            r"^look(?:\s*=|\.)", code, re.MULTILINE
        ):
            assert namespace["look"] in namespace["photo"].effects.items, (
                "Palette adjustments must edit an effect attached to the displayed photo"
            )
        report = namespace["project"].validate()
        assert report.is_valid, (page.name, index + 1, report.diagnostics)

    if page.stem == "masks":
        assert renders == [
            ("portrait-reveal.mp4", {"backend": "cpu", "overwrite": True}),
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


def test_group_concept_coordinates_and_clocks_match_rendered_output():
    namespace = {}
    page = ROOT / "docs/concepts/groups-and-nesting.md"
    for code in snippets(page):
        exec(code, namespace)
    project = namespace["project"]
    assert project.validate().is_valid
    prepared = project.prepare(backend="cpu")
    # The child's group-local (80, 90) is root pixel (240, 180).
    for seconds, colour in (
        (0.9, "101c27ff"),
        (1.25, "263e4bff"),
        (1.5, "263e4bff"),
        (2.0, "e9bd72ff"),
        (3.6, "263e4bff"),
        (4.2, "101c27ff"),
    ):
        frame = prepared.render_frame_seconds(seconds).to_bytes()
        offset = (180 * 640 + 240) * 4
        assert frame[offset : offset + 4] == bytes.fromhex(colour), seconds


def test_card_study_keeps_source_and_placements_independent():
    from vestra import Circle

    example = runpy.run_path(str(ROOT / "docs/assets/examples/group_placements.py"))
    project = example["project"]
    before = project.snapshot().to_dict()
    # A later source edit must not update either already-placed card.
    example["card"].add(Circle(radius=10, fill="#ffffff"))
    assert project.snapshot().to_dict() == before
    # Editing the right placement must not change the left copy.
    example["right"].opacity = 0.25
    after = project.snapshot().to_dict()
    assert before["visual"]["clips"][0] == after["visual"]["clips"][0]
    assert before["visual"]["clips"][1] != after["visual"]["clips"][1]
    assert project.validate().is_valid
