"""The public showcase repeats both media and authored effects at random times."""

import importlib.util
from pathlib import Path

import pytest


@pytest.fixture(scope="module")
def showcase(tmp_path_factory):
    path = (
        Path(__file__).resolve().parents[1]
        / "examples/showcase/stylized-effects/main.py"
    )
    spec = importlib.util.spec_from_file_location("stylized_showcase", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    directory = tmp_path_factory.mktemp("stylized-showcase")
    module.prepare_assets(directory, (64, 48), 8)
    return module, directory


@pytest.mark.parametrize(
    "look",
    [
        "ascii",
        "custom-ascii",
        "pseudo",
        "halftone",
        "horizontal",
        "vertical",
        "crt",
        "palette",
        "dither",
        "analog-monitor",
        "halftone-print",
        "sorted-neon",
    ],
)
def test_showcase_looks_repeat_media_and_effects_at_random_times(showcase, look):
    module, directory = showcase
    project = module.build_project(look, directory, (64, 48), 8)
    assert project.validate().is_valid
    prepared = project.prepare(backend="cpu")
    later = prepared.render_frame_seconds(4.25).to_bytes()
    early = prepared.render_frame_seconds(0.25).to_bytes()
    assert early == later
    assert len(early) == 64 * 48 * 4
    prepared.render_frame_seconds(2.5)
    assert prepared.render_frame_seconds(0.25).to_bytes() == early


@pytest.mark.parametrize("color_mode", ["monochrome", "source", "palette"])
def test_ascii_generated_video_has_visible_glyphs_without_source_mixing(showcase, color_mode):
    from vestra import Project
    from vestra.effects import Ascii
    from vestra.sources import Video

    module, directory = showcase
    project = Project(size=(64, 48), fps=8, duration=4, base_directory=directory)
    layer = project.root.add(Video("source.mkv", sizing="cover"), duration=4)
    control = project.prepare(backend="cpu").render_frame_seconds(0.25).to_bytes()
    assert max(control[0::4]) > 128
    layer.effects.add(Ascii(color_mode=color_mode, palette=module.PALETTE, source_mix=0))
    prepared = project.prepare(backend="cpu")
    data = prepared.render_frame_seconds(0.25).to_bytes()
    red = data[0::4]
    assert max(red) > 64
    assert len(set(red)) > 8
    assert sum(value > 32 for value in red) / len(red) > 0.01
    assert data != control
    prepared.render_frame_seconds(2.5)
    assert prepared.render_frame_seconds(0.25).to_bytes() == data
