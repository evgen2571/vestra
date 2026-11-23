"""Render glowing stereo waves with left-only, right-only and stereo passages."""

from argparse import ArgumentParser
from array import array
import math
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import wave

from vestra import Project, Spectrum2D, Text

SIZE = (960, 540)
DURATION = 6
SAMPLE_RATE = 48_000


def write_audio(path: Path) -> None:
    samples = array("h")
    for frame in range(SAMPLE_RATE * DURATION):
        time = frame / SAMPLE_RATE
        phrase = int(time // 2)
        local = time % 2
        for channel, notes in enumerate(((190, 413, 715), (287, 630, 1180))):
            value = 0.0
            if (phrase == channel or phrase == 2) and local < 1.4:
                for index, frequency in enumerate(notes):
                    elapsed = local - index * 0.23
                    if elapsed >= 0:
                        envelope = min(1, elapsed / 0.025) * math.exp(-elapsed * 3.8)
                        value += (
                            0.29 * envelope * math.sin(math.tau * frequency * elapsed)
                        )
            samples.append(round(value * 32767))
    if sys.byteorder != "little":
        samples.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setparams((2, 2, SAMPLE_RATE, 0, "NONE", "not compressed"))
        output.writeframes(samples.tobytes())


def build(audio: Path, bars: bool) -> Project:
    project = Project(size=SIZE, fps=30, duration=DURATION, background="#0c1123")
    project.audio.track("music").add(audio)
    project.root.add(Spectrum2D(preset="stereo_glow", style="both" if bars else "wave"))
    for index, label in enumerate(("Left input", "Right input", "Left and right")):
        project.root.add(
            Text(label, font_size=30, fill="#e4edf8"),
            start=index * 2,
            duration=2,
            position=(0.06, 0.14),
            anchor=(0, 0),
        )
    return project


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("stereo-spectrum.mp4"))
    parser.add_argument("--backend", choices=("cpu", "wgpu"), default="cpu")
    parser.add_argument("--wave-only", action="store_true")
    args = parser.parse_args()
    with TemporaryDirectory() as directory:
        audio = Path(directory) / "stereo-phrases.wav"
        write_audio(audio)
        build(audio, bars=not args.wave_only).render(
            args.output, backend=args.backend, overwrite=True
        )


if __name__ == "__main__":
    main()
