"""Small Video source example; provide examples/assets/clip.mp4 before running."""

from pathlib import Path

from vestra import Crossfade, Project, Text, Video

project = Project(
    size=(1280, 720),
    fps=30,
    duration=6,
    base_directory=Path(__file__).resolve().parents[3],
)

video = project.root.add(Video("examples/assets/clip.mp4", sizing="cover"), duration=6)
title = project.root.add(
    Text("Vestra Video", font="tests/assets/VestraTest-Regular.ttf", font_size=42),
    duration=6,
    z=1,
)
project.root.transitions.add(video, title, Crossfade(), start=3, duration=1)

# A second layer can reuse the same Video source with independent media timing.
project.root.add(
    video.source, start=2, source_start=1, duration=3, playback_rate=0.5, z=2
)
