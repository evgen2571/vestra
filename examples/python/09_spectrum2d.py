"""Minimal public Spectrum2D authoring example."""

import video_editor
from video_editor import FrameRate
from video_editor.authoring import ProjectBuilder


project = ProjectBuilder(
    width=320, height=180, frame_rate=FrameRate(30, 1), output_path="spectrum2d.mp4",
    duration=3.0, background="#101018", base_directory=".",
)
background_asset = project.add_image_asset("examples/assets/green.png")
project.add_image_clip(source=background_asset, start=0, duration=3, layer=0)
audio_asset = project.add_audio_asset("examples/assets/tone.wav")
audio_track = project.audio.add_track(id="music")
audio_track.add_clip(asset=audio_asset, start=0, trim_end=3)
spectrum = project.add_spectrum2d_clip(start=0, duration=3, layer=1)
spectrum.effects.add_bloom(threshold=0.5, radius=4.0, intensity=0.8)

video_editor.Editor().render(
    project.build(),
    video_editor.RenderRequest(
        "spectrum2d.mp4", backend=video_editor.BackendPreference.CPU, overwrite=True,
    ),
)
