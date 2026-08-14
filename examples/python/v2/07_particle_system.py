"""Author and preview a procedural particle source through the high-level API."""

from __future__ import annotations

from pathlib import Path

from vestra import Project
from vestra.sources import ParticleSystem, PointEmitter

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(64, 64), fps=12, duration=1, base_directory=ROOT)
    particles = ParticleSystem(
        emitter=PointEmitter(),
        rate=24,
        seed=7,
        lifetime=0.75,
        size=0.08,
        colour="#ff8040",
        speed=0.25,
        direction=-90,
        spread=120,
    )
    project.root.add(particles, duration=1, id="sparks")

    report = project.validate()
    if not report.is_valid:
        raise SystemExit("particle project validation failed")
    frame = project.prepare(backend="cpu").render_frame_seconds(0.5)
    print(frame.frame_number, len(frame.to_bytes()))


if __name__ == "__main__":
    main()
