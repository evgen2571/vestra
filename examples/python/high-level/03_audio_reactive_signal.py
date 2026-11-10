"""Bind a master-audio signal to ordinary layer and effect properties.

Run from the repository root: uv run python examples/python/high-level/03_audio_reactive_signal.py
"""

from __future__ import annotations

from pathlib import Path

from vestra import Project
from vestra.effects import Bloom
from vestra.sources import Color

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(160, 90), fps=12, duration=1, base_directory=ROOT)
    project.audio.track("music").add("assets/fixtures/tone.wav", trim_end=1.0)
    background = project.root.add(Color("#304060"), duration=1)

    bloom = background.effects.add(Bloom(threshold=0.5, radius=2.0, intensity=0.2))

    # Restrict the bass measurement before using it as a visual multiplier.
    bass_response = (
        project.audio.signal.band_energy(40, 160)
        .gain(1.8)
        .remap(input=(0.0, 0.2), output=(0.0, 1.0))
        .clamp(0.0, 1.0)
        .envelope(0.025, 0.18)
    )

    background.transform.scale.bind(bass_response, operation="multiply")
    bloom.intensity.bind(bass_response, operation="replace")
    report = project.validate()
    if not report.is_valid:
        raise SystemExit("audio-reactive project validation failed")
    print("Bass signal attached to background scale and bloom intensity")


if __name__ == "__main__":
    main()
