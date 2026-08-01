"""Build, inspect, prepare, and render a small schema-v2 music mix."""
from pathlib import Path

from video_editor import BackendPreference, Editor, FrameRate, PrepareOptions, PreparedVideoRenderRequest
from video_editor.authoring import AudioFadeCurve, AudioGainInterpolation, AudioGainKeyframe, ProjectBuilder

root = Path(__file__).resolve().parents[2]
builder = ProjectBuilder(
    width=320, height=180, frame_rate=FrameRate(30, 1), output_path="music-mix.mp4",
    duration=3.0, output_audio=True, base_directory=root,
)
builder.add_solid_color_clip(colour="#101018", start=0, duration=3, layer=0)
song_a = builder.add_audio_asset("examples/assets/tone.wav", id="song-a")
song_b = builder.add_audio_asset("examples/assets/tone.wav", id="song-b")
ambience = builder.add_audio_asset("examples/assets/tone.wav", id="ambience-source")

music = builder.audio.add_track(id="music", gain=1.0)
background = builder.audio.add_track(id="ambience", gain=0.2)
outgoing = music.add_clip(asset=song_a, start=0, trim_end=2.0, gain=0.8)
incoming = music.add_clip(asset=song_b, start=1.5, trim_end=1.5, gain=0.9)
incoming.set_gain_automation([
    AudioGainKeyframe(0.0, 0.0, AudioGainInterpolation.LINEAR),
    AudioGainKeyframe(0.25, 1.0, AudioGainInterpolation.HOLD),
    AudioGainKeyframe(0.50, 0.8, AudioGainInterpolation.LINEAR),
])
builder.audio.crossfade(outgoing, incoming, curve=AudioFadeCurve.EQUAL_POWER)
background.add_clip(asset=ambience, start=0, trim_end=3.0)

project = builder.build()
validation = builder.validate()
assert validation.is_valid and not validation.warnings
editor = Editor()
inspection = editor.inspect(project)
assert inspection.audio is not None and inspection.audio.track_count == 2
prepared = editor.prepare(project, PrepareOptions(backend=BackendPreference.CPU))
result = prepared.render_video(PreparedVideoRenderRequest(root / "examples/output/music-mix.mp4", overwrite=True))
assert result.audio_present
