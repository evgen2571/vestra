"""Render a typewriter title and styled subtitles from a plain-text SRT."""

from pathlib import Path
from tempfile import TemporaryDirectory

from vestra import (
    Circle,
    Project,
    Rectangle,
    ScalarProperty,
    Subtitles,
    Text,
    WordHighlight,
    WordTiming,
)

SRT = """1
00:00:00,250 --> 00:00:01,350
The title appears one cluster at a time.

2
00:00:01,350 --> 00:00:02,550
The caption box grows upward
when the text needs two lines.

3
00:00:02,750 --> 00:00:04,250
Again and again
"""


def build_project(subtitles: Subtitles) -> Project:
    project = Project(
        size=(640, 360), fps=(30_000, 1001), duration=4.5, background="#203655"
    )
    project.root.add(
        Rectangle(width=640, height=240, fill="#4e807e"), position=(0.5, 0.72)
    )
    project.root.add(Circle(radius=92, fill="#efbc83"), position=(0.78, 0.66))

    reveal = ScalarProperty(0.0)
    reveal.keyframe(0.2, 0.0)
    reveal.keyframe(1.4, 1.0)
    project.root.add(
        Text("Field notes", font_size=60, reveal=reveal), position=(0.5, 0.22)
    )

    style = {
        "font_size": 28,
        "max_width": 520,
        "fill": "#fff9ef",
        "stroke_color": "#172331",
        "stroke_width": 2,
        "background_color": "#10233bb8",
        "background_padding": 10,
    }
    position = (0.5, 0.9)
    captions = Subtitles(subtitles.cues[:-1]).to_group(position=position, **style)

    cue = subtitles.cues[-1]
    # Both instances of "again" have their own character range and local timing.
    timing = [WordTiming(0.0, 0.65, 0, 5), WordTiming(0.9, 1.5, 10, 15)]
    captions.add(
        Text(
            cue.text,
            align="center",
            word_highlight=WordHighlight("#ffd470", timing),
            **style,
        ),
        start=cue.start,
        duration=cue.end - cue.start,
        position=position,
        anchor=(0.5, 1.0),
    )
    project.root.add(captions, duration=subtitles.duration)
    return project


if __name__ == "__main__":
    with TemporaryDirectory() as directory:
        srt_path = Path(directory) / "captions.srt"
        srt_path.write_text(SRT, encoding="utf-8")
        subtitles = Subtitles.from_srt(srt_path)

    project = build_project(subtitles)
    result = project.render(
        "styled-subtitles.mp4", subtitles=subtitles.cues, backend="cpu", overwrite=True
    )
    print(f"Rendered {result.output_path}")
