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
