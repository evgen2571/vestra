"""Author a small particle scene and an audio-reactive appearance property."""

from vestra import FrameRate
from vestra.authoring import (
    ParticleAudioReactive,
    ParticleSystem,
    PointEmitter,
    ProjectBuilder,
    ScalarSignal,
    sparks,
)

project = ProjectBuilder(
    width=64,
    height=64,
    frame_rate=FrameRate(30, 1),
    output_path="particles.mp4",
    duration=1.0,
)
audio_asset = project.add_audio_asset("examples/assets/tone.wav")
project.audio.add_track(id="music").add_clip(asset=audio_asset, start=0, trim_end=1.0)
project.add_particle_system_clip(
    particle_system=sparks(), start=0, duration=1.0, layer=1
)

size = project.scalar_property(1.0)
size.modulate(ScalarSignal({"type": "rms"}).clamp(0.0, 1.0), mode="multiply")
project.add_particle_system_clip(
    particle_system=ParticleSystem(
        emitter=PointEmitter(),
        rate=8.0,
        audio_reactive=ParticleAudioReactive(size=size),
    ),
    start=0,
    duration=1.0,
    layer=2,
)

assert project.validate().is_valid
print(project.to_dict())
