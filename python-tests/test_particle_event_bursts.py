import runpy
from pathlib import Path

import numpy as np
import pytest

from vestra import Project
from vestra.sources import ParticleBurst, ParticleSystem


EXAMPLE = (
    Path(__file__).resolve().parents[1]
    / "examples/python/high-level/08_particle_emission.py"
)


def test_prepared_event_times_become_sorted_clip_local_bursts() -> None:
    times = [2.0, 1.5, 0.99, 1.0000000002, 1.5, 1.0, 1.9999999999]
    expected = (ParticleBurst(0.0, 4), ParticleBurst(0.5, 4))

    for events in (times, reversed(times)):
        assert (
            ParticleBurst.from_project_times(
                events, clip_start=1.0, clip_duration=1.0, count=2
            )
            == expected
        )


@pytest.mark.parametrize("times", [[float("nan")], [float("inf")], [True]])
def test_event_times_must_be_finite_numbers(times) -> None:
    with pytest.raises((ValueError, TypeError), match="event time"):
        ParticleBurst.from_project_times(
            times, clip_start=0.0, clip_duration=1.0, count=1
        )


def test_event_bursts_use_existing_live_particle_validation() -> None:
    project = Project(size=(32, 32), fps=4, duration=2)
    bursts = ParticleBurst.from_project_times(
        [1.0, 1.0, 1.0], clip_start=1.0, clip_duration=1.0, count=100_000
    )
    project.root.add(ParticleSystem(bursts=bursts), start=1, duration=1)

    assert bursts == (ParticleBurst(0.0, 300_000),)
    assert any(
        error.code == "VESTRA-LIMIT-PARTICLES" for error in project.validate().errors
    )


def test_generated_clicks_match_burst_frames_when_seeking(tmp_path: Path) -> None:
    example = runpy.run_path(str(EXAMPLE))
    project = example["click_project"](tmp_path / "clicks.wav")
    example["add_click_bursts"](project)
    prepared = project.prepare(backend="cpu")

    order = (11, 12, 14, 15, 20, 21, 24, 30, 33, 35, 36)
    frames = {index: prepared.render_frame_number(index).to_bytes() for index in order}
    background = frames[11]
    assert {index for index, frame in frames.items() if frame != background} == {
        12,
        14,
        21,
        30,
    }
    for index in (*reversed(order), 12, 36, 12):
        assert prepared.render_frame_number(index).to_bytes() == frames[index]


def test_emission_example_at_clip_and_lifetime_boundaries(tmp_path: Path) -> None:
    example = runpy.run_path(str(EXAMPLE))
    project = example["build_scene"](tmp_path / "clicks.wav")
    prepared = project.prepare(backend="cpu")

    def contains(frame: int, rgb: tuple[int, int, int]) -> bool:
        pixels = prepared.render_frame_number(frame).to_numpy()
        return bool(np.any(np.all(pixels == (*rgb, 255), axis=-1)))

    snow = (232, 243, 255)
    trail = (103, 232, 237)
    bursts = (255, 182, 92)

    assert contains(0, snow)
    assert not contains(6, trail)
    assert contains(7, trail)
    assert contains(41, trail)
    assert not contains(42, trail)
    assert contains(12, bursts)
    assert contains(14, bursts)
    assert not contains(15, bursts)
    assert not contains(36, bursts)


def test_event_on_frame_boundary_with_fractional_clip_start() -> None:
    start = 0.5e-9
    project = Project(size=(32, 32), fps=2, duration=1.5)
    bursts = ParticleBurst.from_project_times(
        [0.5], clip_start=start, clip_duration=1.4, count=1
    )
    project.root.add(
        ParticleSystem(bursts=bursts, lifetime=0.25, size=0.5),
        start=start,
        duration=1.4,
    )
    prepared = project.prepare(backend="cpu")
    before = prepared.render_frame_number(0).to_bytes()

    assert prepared.render_frame_number(1).to_bytes() != before
    assert prepared.render_frame_number(2).to_bytes() == before
