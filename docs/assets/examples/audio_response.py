"""Silent audio studies. Run: python audio_response.py (replaces WAV and MP4s)."""

from array import array
import math
from pathlib import Path
import sys
import wave

from vestra import Circle, Line, Polygon, Project, Rectangle, Spectrum2D, Text
from vestra.sources import (
    Spectrum2DGradient,
    Spectrum2DLinearLayout,
    Spectrum2DRadialLayout,
)

DURATION = 4
FPS = 30
SIZE = (640, 560)
SAMPLE_RATE = 48_000
PULSE_STARTS = (0.35, 1.15, 1.95, 2.75, 3.55)
RMS_MAX = 0.18
LEFT, RIGHT = 32, 608
RAW, FAST, SLOW = "#f5ad76", "#91e2ca", "#bdb4ff"


def write_pulses(path: Path) -> None:
    """Original bass-and-melody phrase; no recording or music download."""
    samples = array("h")
    for frame in range(SAMPLE_RATE * DURATION):
        time = frame / SAMPLE_RATE
        sample = 0.0
        for index, start in enumerate(PULSE_STARTS):
            elapsed = time - start
            if 0 <= elapsed < 0.34:
                envelope = min(elapsed / 0.008, 1) * math.exp(-elapsed / 0.10)
                amplitude = (0.50, 0.80, 0.65, 0.90, 0.70)[index]
                frequency = (220, 330, 660, 990, 1320)[index]
                tone = 0.45 * math.sin(2 * math.pi * 110 * elapsed)
                tone += 0.40 * math.sin(2 * math.pi * frequency * elapsed)
                tone += 0.15 * math.sin(2 * math.pi * frequency * 1.5 * elapsed)
                sample += amplitude * envelope * tone
        samples.append(round(sample * 32767))
    if sys.byteorder != "little":
        samples.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setparams((1, 2, SAMPLE_RATE, 0, "NONE", "not compressed"))
        output.writeframes(samples.tobytes())


def responses(project: Project):
    raw = project.audio.signal.rms().remap(input=(0, RMS_MAX), output=(0, 1))
    raw = raw.clamp(0, 1)
    return (raw, raw.envelope(0.01, 0.20), raw.envelope(0.15, 0.20))


def sample_responses(audio_path: Path) -> tuple[list[float], ...]:
    """Read actual prepared signal values through subpixel marker positions."""
    probe = Project(size=(12, 1024), fps=FPS, duration=DURATION, output_audio=False)
    probe.audio.track("input").add(audio_path, trim_end=DURATION)
    for index, signal in enumerate(responses(probe)):
        marker = probe.root.add(
            Rectangle(width=2, height=2, fill="#ffffff"),
            position=((index * 4 + 2) / 12, 0.5),
        )
        marker.transform.position_y.bind(
            signal.remap(input=(0, 1), output=(0.9, 0.1)), operation="replace"
        )
    prepared = probe.prepare(backend="cpu")
    values = ([], [], [])
    for frame_number in range(DURATION * FPS):
        pixels = prepared.render_frame_number(frame_number).to_bytes()
        for index, series in enumerate(values):
            weights = [pixels[(row * 12 + index * 4 + 2) * 4] for row in range(1024)]
            weight = sum(weights)
            if not weight:
                raise RuntimeError("Prepared signal marker is outside the probe")
            centre = sum((row + 0.5) * value for row, value in enumerate(weights))
            value = (0.9 - centre / weight / 1024) / 0.8
            series.append(max(0.0, min(1.0, value)))
    return values


def label(project, text, x, y, size=24, colour="#edf3f5", anchor=(0, 0.5)):
    project.root.add(
        Text(text, font_size=size, fill=colour),
        position=(x / SIZE[0], y / SIZE[1]),
        anchor=anchor,
        z=5,
    )


def shape(project, source, x, y, **kwargs):
    return project.root.add(source, position=(x / SIZE[0], y / SIZE[1]), **kwargs)


def playhead(project, top, bottom, *, colour="#e9f1f5", id="playhead"):
    cursor = shape(
        project,
        Line(start=(0, 0), end=(0, bottom - top), stroke=colour, stroke_width=1.5),
        LEFT,
        (top + bottom) / 2,
        z=3,
        id=id,
    )
    cursor.transform.position.keyframe(
        0, (LEFT / SIZE[0], (top + bottom) / 2 / SIZE[1])
    )
    cursor.transform.position.keyframe(
        DURATION, (RIGHT / SIZE[0], (top + bottom) / 2 / SIZE[1])
    )


def waveform(project: Project, audio_path: Path) -> None:
    with wave.open(str(audio_path), "rb") as recording:
        samples = array("h", recording.readframes(recording.getnframes()))
    if sys.byteorder != "little":
        samples.byteswap()
    peaks = [
        max(abs(value) for value in samples[start : start + SAMPLE_RATE // 120]) / 32768
        for start in range(0, len(samples), SAMPLE_RATE // 120)
    ]
    width = RIGHT - LEFT
    top = [
        (index / (len(peaks) - 1) * width, 18 - peak * 28)
        for index, peak in enumerate(peaks)
    ]
    bottom = [(x, 36 - y) for x, y in reversed(top)]
    shape(
        project,
        Polygon(points=top + bottom, fill="#607888"),
        LEFT,
        100,
        anchor=(0, 0.5),
        id="input-waveform",
    )
    shape(
        project,
        Line(start=(0, 0), end=(width, 0), stroke="#3b5260", stroke_width=1),
        LEFT,
        100,
        anchor=(0, 0.5),
    )
    playhead(project, 82, 118, colour=FAST, id="input-playhead")


def curve(project, values, signal, top, bottom, colour, name):
    width, height = RIGHT - LEFT, bottom - top
    points = [
        (index / (FPS * DURATION) * width, height * (1 - value))
        for index, value in enumerate(values)
    ]
    strip = [(x, y - 1.5) for x, y in points]
    strip += [(x, y + 1.5) for x, y in reversed(points)]
    # Shape sources rebase their bounds; retain the measured vertical offset.
    shape(
        project,
        Polygon(points=strip, fill=colour),
        LEFT,
        top + min(y for _, y in strip),
        anchor=(0, 0),
        id=f"curve-{name}",
        z=2,
    )
    area = [(0, height), *points, (points[-1][0], height)]
    shape(
        project,
        Polygon(points=area, fill=colour),
        LEFT,
        top + min(y for _, y in area),
        anchor=(0, 0),
        opacity=0.12,
    )
    shape(
        project,
        Line(start=(0, 0), end=(width, 0), stroke="#3b5260", stroke_width=1),
        LEFT,
        bottom,
        anchor=(0, 0.5),
    )
    dot = shape(
        project, Circle(radius=5, fill=colour), LEFT, bottom, z=4, id=f"dot-{name}"
    )
    dot.transform.position.keyframe(0, (LEFT / SIZE[0], bottom / SIZE[1]))
    dot.transform.position.keyframe(DURATION, (RIGHT / SIZE[0], bottom / SIZE[1]))
    dot.transform.position_y.bind(
        signal.remap(input=(0, 1), output=(bottom / SIZE[1], top / SIZE[1])),
        operation="replace",
    )


def build_project(audio_path: Path, study: str) -> Project:
    if study not in ("bounded", "smoothing", "spectra"):
        raise ValueError(f"Unknown study: {study}")
    project = Project(size=SIZE, fps=FPS, duration=DURATION, background="#101c27")
    project.audio.track("analysis input").add(audio_path, trim_end=DURATION)
    project.output_audio = False
    # Reserve the lower 120px for native video controls at mobile widths.
    shape(
        project, Rectangle(width=640, height=120, fill="#0b141c"), 0, 440, anchor=(0, 0)
    )
    shape(
        project,
        Line(start=(0, 0), end=(640, 0), stroke="#253947", stroke_width=2),
        0,
        440,
        anchor=(0, 0.5),
        z=1,
    )
    raw, fast, slow = responses(project)
    waveform(project, audio_path)

    if study == "bounded":
        label(project, "Keep the response in range", LEFT, 34, 30)
        label(project, "RMS → remap → clamp → envelope", LEFT, 68, 24, "#aabcc8")
        for radius in (62 * 0.75, 62 * 1.30):
            shape(
                project,
                Circle(radius=radius, stroke="#607888", stroke_width=2),
                174,
                218,
                z=2,
            )
        pulse = shape(
            project, Circle(radius=62, fill=FAST), 174, 218, z=1, id="bounded-circle"
        )
        pulse.transform.scale.bind(
            fast.remap(input=(0, 1), output=(0.75, 1.30)), operation="multiply"
        )
        label(project, "0.75× minimum", 320, 186)
        label(project, "1.30× maximum", 320, 226)
        label(project, "Scale multiplier", LEFT, 321)
        curve(project, sample_responses(audio_path)[1], fast, 346, 388, FAST, "bounded")
        playhead(project, 337, 397)
    elif study == "smoothing":
        label(project, "How smoothing changes a beat", LEFT, 34, 30)
        label(
            project, "Same RMS · same range · release 0.20 s", LEFT, 68, 24, "#aabcc8"
        )
        for name, title, signal, values, top, colour in zip(
            ("raw", "fast", "slow"),
            ("Raw RMS · no envelope", "Fast attack · 0.01 s", "Slow attack · 0.15 s"),
            (raw, fast, slow),
            sample_responses(audio_path),
            (160, 244, 328),
            (RAW, FAST, SLOW),
        ):
            label(project, title, LEFT, top - 16, 24, colour)
            curve(project, values, signal, top, top + 44, colour, name)
        playhead(project, 127, 385)
    else:
        label(project, "One analysis, two layouts", LEFT, 34, 30)
        label(
            project,
            "16 bands · 80–2400 Hz · identical response",
            LEFT,
            68,
            24,
            "#aabcc8",
        )
        for name, layout, x, y, width, height in (
            ("linear", Spectrum2DLinearLayout(), 32, 195, 304, 174),
            (
                "radial",
                Spectrum2DRadialLayout(inner_radius_ratio=0.44),
                378,
                179,
                230,
                230,
            ),
        ):
            project.root.add(
                Spectrum2D(
                    band_count=16,
                    min_hz=80,
                    max_hz=2400,
                    sensitivity=250,
                    attack_seconds=0.01,
                    release_seconds=0.18,
                    x=x / SIZE[0],
                    y=y / SIZE[1],
                    width=width / SIZE[0],
                    height=height / SIZE[1],
                    layout=layout,
                    colour=FAST,
                    gradient=Spectrum2DGradient(FAST, SLOW, "across_bands"),
                    bar_gap_ratio=0.24,
                    min_bar_height_ratio=0.015,
                ),
                id=f"spectrum-{name}",
            )
        label(project, "Linear", LEFT, 154)
        label(project, "Radial", 417, 154)
        label(project, "Low → high", LEFT, 398, 24, "#aabcc8")
        label(project, "Clockwise", 432, 424, 24, "#aabcc8")
    if study != "spectra":
        label(project, "0 s", LEFT, 420, 24, "#aabcc8")
        label(project, "4 s", RIGHT, 420, 24, "#aabcc8", anchor=(1, 0.5))
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
