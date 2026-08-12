from vestra import FrameRate
from vestra.authoring import AudioGainKeyframe, ProjectBuilder

builder = ProjectBuilder(width=10, height=10, frame_rate=FrameRate(30, 1), output_path="out.mp4")
asset = builder.add_audio_asset("tone.wav")
clip = builder.audio.add_track().add_clip(asset=asset, start=0)
clip.set_gain_automation([AudioGainKeyframe(time="zero", gain="one")])
clip.set_gain_automation(["not a keyframe"])
builder.audio.crossfade(clip, asset)
