"""Render pre-rolled snow, a moving emitter, and bursts timed to a click track."""

from __future__ import annotations

import argparse
import math
import wave
from dataclasses import replace
from pathlib import Path
from struct import pack
from tempfile import TemporaryDirectory

from vestra import Point, Project
from vestra.authoring import PointKeyframe, snow
from vestra.sources import ParticleBurst, ParticleSystem, PointEmitter

CLICK_TIMES = (0.25, 0.5, 0.875, 1.25, 1.5)
FPS = 24
DURATION = 2.0


def write_click_track(path: Path) -> None:
    sample_rate = 48_000
    samples = bytearray(int(DURATION * sample_rate) * 2)
    for time in CLICK_TIMES:
        start = int(time * sample_rate)
        for offset in range(720):
            envelope = 1 - offset / 720
            tone = math.sin(math.tau * 1400 * offset / sample_rate)
            sample = round(24_000 * envelope * tone)
            index = 2 * (start + offset)
            samples[index : index + 2] = pack("<h", sample)
    with wave.open(str(path), "wb") as output:
        output.setparams((1, 2, sample_rate, 0, "NONE", "not compressed"))
        output.writeframes(samples)


def click_project(audio: Path) -> Project:
    write_click_track(audio)
    project = Project(
        size=(256, 144),
        fps=FPS,
        duration=DURATION,
        background="#102031",
        base_directory=audio.parent,
    )
    project.audio.track("clicks").add(audio, start=0.0)
    return project


def add_click_bursts(project: Project) -> None:
    start, duration = 0.5, 1.0
    # The synthetic track supplies repeatable project-time onset data.
    bursts = ParticleBurst.from_project_times(
        CLICK_TIMES, clip_start=start, clip_duration=duration, count=12
    )
    project.root.add(
        ParticleSystem(
            emitter=PointEmitter(Point(0.5, 0.73)),
            bursts=bursts,
            seed=23,
            lifetime=0.125,
            size=0.05,
            speed=0.85,
            spread=360,
            colour="#ffb65c",
        ),
        id="click-bursts",
        start=start,
        duration=duration,
    )


def build_scene(audio: Path) -> Project:
    project = click_project(audio)
    flurries = ParticleSystem(replace(snow(density=32), pre_roll=5.0))
    project.root.add(flurries, id="snow", duration=DURATION)
    project.root.add(
        ParticleSystem(
            emitter=PointEmitter(Point(0.15, 0.28)),
            emitter_keyframes=(
                PointKeyframe(time=0.0, value=Point(0.15, 0.28)),
                PointKeyframe(time=1.25, value=Point(0.85, 0.28)),
            ),
            rate=28,
            lifetime=0.55,
            size=0.035,
            colour="#67e8ed",
            initial_velocity=Point(0.0, 0.26),
        ),
        id="moving-emitter",
        start=0.25,
        duration=1.5,
    )
    add_click_bursts(project)
    return project


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--backend", choices=("cpu", "wgpu"), default="cpu")
    args = parser.parse_args()

    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory() as directory:
        audio = Path(directory) / "clicks.wav"
        build_scene(audio).render(output, backend=args.backend)


if __name__ == "__main__":
    main()
