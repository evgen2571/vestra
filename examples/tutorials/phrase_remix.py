"""Remix a generated stereo phrase at 120 BPM."""

from argparse import ArgumentParser
from array import array
import math
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import wave

from vestra import (
    AudioFilterKeyframe,
    BeatRepeat,
    FilterSweep,
    PitchShift,
    Project,
    Spectrum2D,
    TapeStop,
)

SAMPLE_RATE = 48_000
BPM = 120
BEAT = 60 / BPM
CHORUS_START = 4 * BEAT
TURNAROUND_START = 6 * BEAT
NEXT_DOWNBEAT = 8 * BEAT
DURATION = 9 * BEAT


def write_stereo_mix(path: Path) -> None:
    """Write a bass line and a stereo lead with a distinct attack on each beat."""
    samples = array("h")
    for note in (60, 64, 67, 72, 69, 72, 76, 72, 69):
        frequency = 440 * 2 ** ((note - 69) / 12)
        for frame in range(round(BEAT * SAMPLE_RATE)):
            elapsed = frame / SAMPLE_RATE
            edge = min(elapsed / 0.005, (BEAT - elapsed) / 0.02, 1.0)
            attack = math.exp(-14 * elapsed)
            phase = math.tau * frequency * elapsed
            bass = 0.22 * math.sin(math.tau * 82 * elapsed)
            lead = (0.19 + 0.08 * attack) * math.sin(phase)
            brightness = 0.07 + 0.15 * attack
            left = bass + lead + brightness * math.sin(3 * phase)
            right = bass + lead + brightness * math.sin(4 * phase)
            samples.extend(round(value * edge * 32767) for value in (left, right))

    if sys.byteorder != "little":
        samples.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setparams((2, 2, SAMPLE_RATE, 0, "NONE", "not compressed"))
        output.writeframes(samples.tobytes())


def build_project(audio: Path) -> Project:
    project = Project(size=(480, 270), fps=30, duration=DURATION, background="#10172a")
    music = project.audio.track("music")

    lead_in = music.add(audio, trim_end=CHORUS_START)
    last_beat = CHORUS_START - BEAT
    sixteenth = 15 / BPM
    lead_in.effects.extend(
        [
            FilterSweep(
                mode="high_pass",
                keyframes=[
                    AudioFilterKeyframe(time=0, frequency_hz=20),
                    AudioFilterKeyframe(time=last_beat, frequency_hz=900),
                ],
            ),
            BeatRepeat(
                slice_start=last_beat,
                slice_duration=sixteenth,
                output_start=last_beat,
                repeats=4,
            ),
        ]
    )

    # The chorus is a separate clip so the downbeat has its full bass again.
    chorus = music.add(
        audio, start=CHORUS_START, trim_start=CHORUS_START, trim_end=TURNAROUND_START
    )
    chorus.effects.add(PitchShift(semitones=2))

    turnaround = music.add(
        audio,
        start=TURNAROUND_START,
        trim_start=TURNAROUND_START,
        trim_end=NEXT_DOWNBEAT,
    )
    turnaround.effects.add(TapeStop(start=0.25, duration=0.375))
    music.add(audio, start=NEXT_DOWNBEAT, trim_start=NEXT_DOWNBEAT, trim_end=DURATION)

    project.root.add(
        Spectrum2D(
            min_hz=65,
            max_hz=5000,
            band_count=32,
            sensitivity=2.0,
            response_power=0.5,
            attack_seconds=0,
            release_seconds=0.02,
            x=0.10,
            y=0.20,
            width=0.80,
            height=0.60,
            colour="#8de5fa",
        )
    )
    return project


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("phrase-remix.mp4"))
    args = parser.parse_args()

    with TemporaryDirectory() as directory:
        source = Path(directory) / "stereo-mix.wav"
        write_stereo_mix(source)
        result = build_project(source).render(
            args.output, backend="cpu", overwrite=True
        )
        print(f"Rendered {result.output_path} with {result.selected_backend}")


if __name__ == "__main__":
    main()
