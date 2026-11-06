"""Generate a pulse recording and three audible studies. Run: python audio_response.py"""

from array import array
import math
from pathlib import Path
import sys
import wave

from vestra import Circle, Project, Spectrum2D, Text
from vestra.sources import Spectrum2DLinearLayout, Spectrum2DRadialLayout

DURATION = 3
SAMPLE_RATE = 48_000
PULSE_STARTS = (0.25, 0.85, 1.45, 2.05, 2.65)


def write_pulses(path: Path) -> None:
    """Original deterministic tones; no external recording or music download."""
    samples = array("h")
    for frame in range(SAMPLE_RATE * DURATION):
        time = frame / SAMPLE_RATE
        sample = 0.0
        for index, start in enumerate(PULSE_STARTS):
            elapsed = time - start
            if 0 <= elapsed < 0.24:
                envelope = min(elapsed / 0.006, 1) * math.exp(-elapsed / 0.075)
                amplitude = (0.35, 0.60, 0.45, 0.70, 0.50)[index]
                frequency = (220, 440, 880, 1760, 440)[index]
                tone = 0.65 * math.sin(2 * math.pi * 110 * elapsed)
                tone += 0.35 * math.sin(2 * math.pi * frequency * elapsed)
                sample += amplitude * envelope * tone
        samples.append(round(sample * 32767))
    if sys.byteorder != "little":
        samples.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setparams((1, 2, SAMPLE_RATE, 0, "NONE", "not compressed"))
        output.writeframes(samples.tobytes())


def label(
    project: Project, text: str, position: tuple[float, float], size: int = 22
) -> None:
    project.root.add(Text(text, font_size=size, fill="#eff4ec"), position=position, z=2)


def build_project(audio_path: Path, study: str) -> Project:
    project = Project(size=(640, 360), fps=30, duration=DURATION, background="#101c24")
    project.audio.track("pulses").add(audio_path, trim_end=DURATION)
    energy = project.audio.signal.rms()
    bounded_scale = energy.remap(input=(0, 0.18), output=(0.75, 1.30)).clamp(0.75, 1.30)

    if study == "bounded":
        label(project, "RMS drives a bounded scale", (0.5, 0.12), 25)
        for radius in (70 * 0.75, 70 * 1.30):
            project.root.add(
                Circle(radius=radius, stroke="#6c827e", stroke_width=2),
                position=(0.5, 0.52),
            )
        pulse = project.root.add(
            Circle(radius=70, fill="#9edbc7"), position=(0.5, 0.52)
        )
        pulse.transform.scale.bind(
            bounded_scale.envelope(0.01, 0.15), operation="multiply"
        )
        label(project, "0.75 minimum / 1.30 maximum", (0.5, 0.23), 21)
    elif study == "smoothing":
        label(project, "Same RMS and range; different smoothing", (0.5, 0.10), 24)
        label(project, "Attack / release in seconds", (0.5, 0.19), 19)
        responses = (
            ("No envelope", None),
            ("Fast attack", (0.01, 0.20)),
            ("Slow attack", (0.15, 0.20)),
        )
        for index, (name, timings) in enumerate(responses):
            x = 0.19 + index * 0.31
            label(project, name, (x, 0.30), 22)
            pulse = project.root.add(
                Circle(radius=48, fill="#9edbc7"), position=(x, 0.60)
            )
            response = (
                bounded_scale if timings is None else bounded_scale.envelope(*timings)
            )
            pulse.transform.scale.bind(response, operation="multiply")
            detail = (
                "raw RMS"
                if timings is None
                else f"{timings[0]:.2f} / {timings[1]:.2f} s"
            )
            label(project, detail, (x, 0.39), 20)
    elif study == "spectra":
        label(project, "Same 16 bands / 80-2400 Hz", (0.5, 0.11), 25)
        for layout, x, width in (
            (Spectrum2DLinearLayout(), 0.06, 0.42),
            (Spectrum2DRadialLayout(inner_radius_ratio=0.45), 0.61, 0.28),
        ):
            project.root.add(
                Spectrum2D(
                    band_count=16,
                    min_hz=80,
                    max_hz=2400,
                    sensitivity=350,
                    attack_seconds=0.01,
                    release_seconds=0.18,
                    x=x,
                    y=0.28,
                    width=width,
                    height=0.50,
                    layout=layout,
                    colour="#9edbc7",
                    bar_gap_ratio=0.25,
                    min_bar_height_ratio=0.01,
                )
            )
        label(project, "Linear", (0.27, 0.22), 23)
        label(project, "Radial", (0.75, 0.22), 23)
    else:
        raise ValueError(f"Unknown study: {study}")
    return project


def main() -> None:
    audio_path = Path("audio-pulses.wav").resolve()
    write_pulses(audio_path)
    for study in ("bounded", "smoothing", "spectra"):
        project = build_project(audio_path, study)
        result = project.render(f"audio-{study}.mp4", backend="cpu", overwrite=True)
        print(
            f"Rendered {result.output_path}; selected backend: {result.selected_backend}"
        )


if __name__ == "__main__":
    main()
