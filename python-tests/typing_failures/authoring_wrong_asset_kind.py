from video_editor import FrameRate
from video_editor.authoring import ProjectBuilder

builder = ProjectBuilder(
    width=160, height=90, frame_rate=FrameRate(30, 1), output_path="out.mp4", duration=1.0,
)
image = builder.add_image_asset("cover.png")
audio = builder.add_audio_asset("music.wav")
builder.add_image_clip(source=audio, start=0.0, duration=1.0, layer=0)
builder.audio.add_track().add_clip(asset=image, start=0.0)
