"""A complete advanced-authoring example, run from the repository root.

Run from the repository root: uv run python examples/python/authoring/06_full_project.py
"""

from pathlib import Path

from vestra import BackendPreference, Crossfade, Editor, FrameRate, PrepareOptions
from vestra.authoring import BlendMode, Interpolation, Point, ProjectBuilder, Sizing


root = Path(__file__).resolve().parents[3]
builder = ProjectBuilder(
    width=320,
    height=180,
    frame_rate=FrameRate(30, 1),
    output_path="full-project.mp4",
    duration=2.5,
    base_directory=root,
)
# Register inputs once so multiple clips can reuse them.
photo = builder.add_image_asset("tests/assets/wgpu-small-rgba.png")
music_file = builder.add_audio_asset("assets/fixtures/tone.wav")

builder.add_solid_color_clip(colour="#102030", start=0, duration=2.5, layer=-1)
outgoing = builder.add_image_clip(
    source=photo, start=0, duration=2, layer=0, sizing=Sizing.cover()
)
incoming = builder.add_image_clip(
    source=photo, start=1.5, duration=1, layer=1, sizing=Sizing.cover()
)
outgoing.transform.position.keyframe(
    time=0.5, value=Point(0.45, 0.5), interpolation=Interpolation.EASE_OUT
)
outgoing.effects.add_brightness(amount=0.1)
outgoing.blend_mode = BlendMode.SCREEN
outgoing.presets.apply_impact(seed=7, duration=0.4)

# Move the incoming clip, then fit the crossfade to the remaining overlap.
builder.timeline.shift_clip(incoming, delta=0.25)
builder.transitions.add_transition(
    outgoing=outgoing,
    incoming=incoming,
    definition=Crossfade().to_canonical(),
    start=1.75,
    duration=0.25,
)
builder.flashes.add(start=1.8, duration=0.1, colour="#ffffff", opacity=0.4, layer=3)

music = builder.audio.add_track(id="music")
music.add_clip(asset=music_file, start=0, trim_start=0, trim_end=0.2)

assert builder.validate().is_valid
# Prepare once, then request a frame without encoding a video.
project = builder.build()
options = PrepareOptions(backend=BackendPreference.CPU)
prepared = Editor().prepare(project, options)
frame = prepared.render_frame_number(0)
print(f"Prepared frame: {frame.width} × {frame.height} pixels")
