import pytest

import vestra
from vestra.authoring import ParticleSystem, Point, PointKeyframe, ProjectBuilder


def test_particle_emission_controls_lower_in_both_authoring_apis() -> None:
    frames = (
        PointKeyframe(0, Point(0.25, 0.5)),
        PointKeyframe(1, Point(0.75, 0.5)),
    )
    legacy = ParticleSystem().to_canonical()
    assert "emitter_keyframes" not in legacy
    assert legacy["emission"] == {"rate": 0.0, "bursts": []}
    assert vestra.PointKeyframe is PointKeyframe

    definition = ParticleSystem(
        emitter_keyframes=frames, pre_roll=1.0, rate=2, size=0.1
    )
    advanced = ProjectBuilder(
        width=32,
        height=32,
        frame_rate=vestra.FrameRate(4, 1),
        output_path="particles.mp4",
        duration=2,
    )
    advanced.add_particle_system_clip(
        particle_system=definition, start=0.25, duration=1, layer=0
    )
    assert advanced.validate().is_valid

    high_level = vestra.Project(size=(32, 32), fps=4, duration=2)
    source = vestra.ParticleSystem(
        emitter_keyframes=frames, pre_roll=1.0, rate=2, size=0.1
    )
    layer = high_level.root.add(source, start=0.25, duration=1)
    assert isinstance(layer.source, vestra.ParticleSystem)
    assert layer.source.emitter_keyframes == frames
    assert layer.source.pre_roll == 1.0
    snapshot = high_level.snapshot()
    from_high_level = snapshot.to_dict()["visual"]["clips"][0]["source"]
    from_advanced = advanced.build().to_dict()["visual"]["clips"][0]["source"]
    assert from_high_level == from_advanced
    assert from_high_level["emitter_keyframes"] == [
        frame.to_canonical() for frame in frames
    ]
    assert from_high_level["emission"]["pre_roll"] == 1.0

    prepared = vestra.Editor().prepare(
        snapshot, vestra.PrepareOptions(backend=vestra.BackendPreference.CPU)
    )
    before = prepared.render_frame_number(0).to_bytes()
    first = prepared.render_frame_number(1).to_bytes()
    later = prepared.render_frame_number(4).to_bytes()
    after = prepared.render_frame_number(5).to_bytes()
    assert first != before
    assert later != first
    assert after == before
    assert prepared.render_frame_number(1).to_bytes() == first
    assert prepared.render_frame_number(4).to_bytes() == later


@pytest.mark.parametrize("factory", [ParticleSystem, vestra.ParticleSystem])
def test_particle_emission_controls_reject_invalid_authoring(factory: type) -> None:
    with pytest.raises(ValueError, match="pre_roll must be"):
        factory(pre_roll=-1)
    with pytest.raises(TypeError, match="PointKeyframe"):
        factory(emitter_keyframes=(object(),))
    duplicate = PointKeyframe(0.5, Point(0.25, 0.5))
    with pytest.raises(ValueError, match="strictly increasing"):
        factory(emitter_keyframes=(duplicate, duplicate))
