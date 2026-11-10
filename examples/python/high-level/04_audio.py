"""Author a track, clip fades, automation, and supported audio effects.

Run from the repository root: uv run python examples/python/high-level/04_audio.py
"""

from __future__ import annotations

from pathlib import Path

from vestra import Project
from vestra.audio import AudioGainKeyframe, BassBoost

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(160, 90), fps=12, duration=1, base_directory=ROOT)
    music = project.audio.track("music", gain=0.9)
    soundtrack = music.add(
        "assets/fixtures/tone.wav",
        start=0.0,
        trim_end=1.0,
        fade_in=0.1,
        fade_out=0.1,
    )

    # Gain times start at the beginning of this clip, not the source recording.
    soundtrack.set_gain_automation(
        [
            AudioGainKeyframe(0.0, 0.0),
            AudioGainKeyframe(0.5, 1.0),
        ]
    )

    music.effects.add(BassBoost(gain_db=3.0, frequency_hz=100.0))
    project.snapshot()
    print(f"Music track gain: {music.gain}; clip gain: {soundtrack.gain}")


if __name__ == "__main__":
    main()
