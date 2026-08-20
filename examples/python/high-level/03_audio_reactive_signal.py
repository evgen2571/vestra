"""Bind a master-audio signal to ordinary layer and effect properties."""

from __future__ import annotations

from pathlib import Path

from vestra import Project
from vestra.effects import Bloom
from vestra.sources import Color

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(160, 90), fps=12, duration=1, base_directory=ROOT)
    project.audio.track("music").add("examples/assets/tone.wav", trim_end=1.0)
    layer = project.root.add(Color("#304060"), duration=1)
    bloom = layer.effects.add(Bloom(threshold=0.5, radius=2.0, intensity=0.2))
    signal = (
        project.audio.signal.band_energy(40, 160)
        .gain(1.8)
        .remap(input=(0.0, 0.2), output=(0.0, 1.0))
        .clamp(0.0, 1.0)
        .envelope(0.025, 0.18)
    )
    layer.transform.scale.bind(signal, operation="multiply")
    bloom.intensity.bind(signal, operation="replace")
    snapshot = project.snapshot()
    print(snapshot.to_dict()["audio"]["tracks"][0]["id"])


if __name__ == "__main__":
    main()
