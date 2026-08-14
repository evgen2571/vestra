"""Author a track, clip fades, automation, and supported audio effects."""

from __future__ import annotations

from pathlib import Path

from vestra import Project
from vestra.audio import AudioGainKeyframe, BassBoost

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(160, 90), fps=12, duration=1, base_directory=ROOT)
    track = project.audio.track("music", gain=0.9)
    clip = track.add(
        "examples/assets/tone.wav",
        start=0.0,
        trim_end=1.0,
        fade_in=0.1,
        fade_out=0.1,
    )
    clip.set_gain_automation(
        [
            AudioGainKeyframe(0.0, 0.0),
            AudioGainKeyframe(0.5, 1.0),
        ]
    )
    track.effects.add(BassBoost(gain_db=3.0, frequency_hz=100.0))
    print(project.snapshot().to_dict()["audio"]["tracks"][0]["clips"][0]["gain"])


if __name__ == "__main__":
    main()
