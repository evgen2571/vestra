"""Public Spectrum2D integration example: audio-reactive image, radial bars, and Bloom."""

import video_editor
from video_editor import FrameRate
from video_editor.authoring import ProjectBuilder, Sizing


project = ProjectBuilder(
    width=320, height=180, frame_rate=FrameRate(30, 1), output_path="spectrum2d.mp4",
    duration=3.0, background="#101018", base_directory=".",
)
background_asset = project.add_image_asset("examples/assets/green.png")
background = project.add_image_clip(
    source=background_asset, start=0, duration=3, layer=0, sizing=Sizing.cover(),
)
audio_asset = project.add_audio_asset("examples/assets/tone.wav")
audio_track = project.audio.add_track(id="music")
audio_track.add_clip(asset=audio_asset, start=0, trim_end=3)
bass = project.audio.master.rms().remap(0, 1, 1, 1.08).envelope(0.02, 0.15)
background.transform.scale.react_to(bass)
project.add_spectrum2d_clip(start=0, duration=3, layer=1, preset="neon_circle")

video_editor.Editor().render(
    project.build(),
    video_editor.RenderRequest(
        "spectrum2d.mp4", backend=video_editor.BackendPreference.CPU, overwrite=True,
    ),
)
