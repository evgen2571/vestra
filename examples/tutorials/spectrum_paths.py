"""Render five upper-half-ellipse spectra from one synthetic audio track."""

from argparse import ArgumentParser
from array import array
import math
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import wave

from vestra import Ellipse, Group, Project, Rectangle, Spectrum2D, Text
from vestra.sources import Spectrum2DEllipseLayout, Spectrum2DWave

SIZE = (960, 460)
PANEL = (306, 214)
DURATION = 3
REGION = (0.08, 0.31, 0.84, 0.52)
RADII = (0.37, 0.30)


def write_audio(path: Path) -> None:
    rate = 48_000
    samples = array("h")
    for frame in range(rate * DURATION):
        time = frame / rate
        beat = int(time * 3)
        elapsed = time - beat / 3
        envelope = min(elapsed / 0.015, 1) * math.exp(-elapsed * 6)
        frequency = (174, 261, 392, 587, 880)[beat % 5]
        chord = sum(
            math.sin(math.tau * hz * time) for hz in (130, frequency, frequency * 2.1)
        )
        samples.append(round(6500 * envelope * chord))
    if sys.byteorder != "little":
        samples.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setparams((1, 2, rate, 0, "NONE", "not compressed"))
        output.writeframes(samples.tobytes())


def panel(direction, label, clip=None):
    x, y, width, height = REGION
    rx, ry = RADII
    center = (x + width / 2, y + height / 2)
    ellipse_size = (2 * rx * width * PANEL[0], 2 * ry * height * PANEL[1])
    group = Group(size=PANEL)
    group.add(Rectangle(width=PANEL[0], height=PANEL[1], fill="#161e30"))
    group.add(
        Ellipse(
            width=ellipse_size[0],
            height=ellipse_size[1],
            fill="#233149",
            stroke="#51637d",
            stroke_width=1,
        ),
        position=center,
        layer=1,
    )
    spectrum = group.add(
        Spectrum2D(
            layout=Spectrum2DEllipseLayout(
                rx,
                ry,
                start_angle_degrees=270,
                sweep_angle_degrees=180,
                direction=direction,
            ),
            x=x,
            y=y,
            width=width,
            height=height,
            band_count=24,
            min_hz=100,
            max_hz=2400,
            sensitivity=1.65,
            response_power=0.25,
            soft_knee=0.6,
            colour="#8fe8e1",
            style="both",
            wave=Spectrum2DWave(
                fill_opacity=0.20,
                glow_radius=6,
                glow_opacity=0.55,
                bar_opacity=0.42,
            ),
        ),
        layer=2,
    )
    if clip is not None:
        silhouette = Ellipse(
            width=ellipse_size[0],
            height=ellipse_size[1],
            fill="#ffffff",
            position=center,
        )
        spectrum.masks.add(silhouette).invert = clip == "outside"
    group.add(
        Text(label, font_size=20, fill="#e4edf8"),
        position=(0.06, 0.08),
        anchor=(0, 0),
        layer=3,
    )
    return group


def build(audio: Path) -> Project:
    project = Project(size=SIZE, fps=30, duration=DURATION, background="#0a0e19")
    project.audio.track("music").add(audio)
    cases = (
        ("inward", "Inward", None),
        ("outward", "Outward", None),
        ("both", "Both sides", None),
        ("both", "Inside mask", "inside"),
        ("both", "Inverted mask", "outside"),
    )
    for index, (direction, label, clip) in enumerate(cases):
        column, row = index % 3, index // 3
        project.root.add(
            panel(direction, label, clip),
            position=((column + 0.5) / 3, (row + 0.5) / 2),
        )
    return project


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("spectrum-paths.mp4"))
    parser.add_argument("--backend", choices=("cpu", "wgpu"), default="cpu")
    args = parser.parse_args()
    with TemporaryDirectory() as directory:
        audio = Path(directory) / "pulse-chords.wav"
        write_audio(audio)
        build(audio).render(args.output, backend=args.backend, overwrite=True)


if __name__ == "__main__":
    main()
