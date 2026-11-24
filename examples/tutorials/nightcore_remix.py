"""Turn a generated stereo phrase into a subtle nightcore remix."""

from argparse import ArgumentParser
from array import array
import math
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import wave

from vestra import (
    BassBoost,
    FilteredDelay,
    PlaybackSpeed,
    Project,
    RoomReverb,
    Spectrum2D,
)

SAMPLE_RATE = 48_000
SOURCE_BPM = 100
RATE = 1.10
BEAT_SECONDS = 60 / SOURCE_BPM
SOURCE_SECONDS = 4 * BEAT_SECONDS
PHRASE_END_BEAT = 3 * BEAT_SECONDS
NOTE_SECONDS = 0.20


def write_stereo_mix(path: Path) -> None:
    samples = array("h")
    for frequency in (220, 330, 294, 440):
        for frame in range(round(BEAT_SECONDS * SAMPLE_RATE)):
            elapsed = frame / SAMPLE_RATE
            envelope = max(
                0.0, min(elapsed / 0.008, (NOTE_SECONDS - elapsed) / 0.03, 1.0)
            )
            envelope *= math.exp(-elapsed / 0.18)
            phase = math.tau * frequency * elapsed
            centre = 0.28 * math.sin(phase) + 0.12 * math.sin(math.tau * 110 * elapsed)
            left = centre + 0.14 * math.sin(2 * phase)
            right = centre + 0.14 * math.sin(3 * phase)
            samples.extend(round(value * envelope * 32767) for value in (left, right))

    if sys.byteorder != "little":
        samples.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setparams((2, 2, SAMPLE_RATE, 0, "NONE", "not compressed"))
        output.writeframes(samples.tobytes())


def build_project(audio: Path, *, bass_boost: bool = False) -> Project:
    project = Project(size=(480, 270), fps=30, duration=3, background="#111826")

    music = project.audio.track("music")
    mix = music.add(audio, trim_end=SOURCE_SECONDS)
    mix.effects.add(PlaybackSpeed(rate=RATE, link_pitch=True))
    if bass_boost:
        music.effects.add(BassBoost(gain_db=2.0, frequency_hz=110.0))
    music.effects.add(RoomReverb(decay_seconds=0.5, mix=0.03))

    # Align the trimmed last beat with the same beat in the remixed main clip.
    throw = project.audio.track("delay throw", gain=0.25)
    excerpt = throw.add(
        audio,
        start=PHRASE_END_BEAT / RATE,
        trim_start=PHRASE_END_BEAT,
        trim_end=PHRASE_END_BEAT + NOTE_SECONDS,
    )
    excerpt.effects.add(PlaybackSpeed(rate=RATE, link_pitch=True))
    throw.effects.add(
        FilteredDelay(
            delay_ms=30_000 / (SOURCE_BPM * RATE),
            repeats=2,
            mix=1.0,
            cutoff_hz=3000,
        )
    )

    project.root.add(
        Spectrum2D(
            min_hz=80,
            max_hz=4800,
            sensitivity=2.0,
            response_power=0.34,
            release_seconds=0.03,
            x=0.10,
            y=0.18,
            width=0.80,
            height=0.64,
            colour="#a4ebd8",
        )
    )
    return project


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("nightcore-remix.mp4"))
    parser.add_argument(
        "--bass-boost", action="store_true", help="add 2 dB near 110 Hz"
    )
    args = parser.parse_args()

    with TemporaryDirectory() as directory:
        audio = Path(directory) / "stereo-mix.wav"
        write_stereo_mix(audio)
        result = build_project(audio, bass_boost=args.bass_boost).render(
            args.output, backend="cpu", overwrite=True
        )
        print(f"Rendered {result.output_path} with {result.selected_backend}")


if __name__ == "__main__":
    main()
