import json
import math
import runpy
from pathlib import Path

from jsonschema import Draft202012Validator
import pytest

import vestra
from vestra import FrameRate
from vestra.authoring import (
    CircleEmitter,
    ColourLifetimeStop,
    ParticleAudioReactive,
    ParticleBlendMode,
    ParticleBurst,
    ParticleLifetimeStyle,
    ParticlePrimitive,
    ParticleSystem,
    Point,
    PointEmitter,
    ProjectBuilder,
    RectangleEmitter,
    ScalarLifetimeStop,
    ScalarRange,
    ambient_stars,
    embers,
    radial_burst,
    snow,
    sparks,
)


ROOT = Path(__file__).resolve().parents[1]
VALIDATOR = Draft202012Validator(
    json.loads((ROOT / "schemas/project.schema.json").read_text())
)


def builder() -> ProjectBuilder:
    return ProjectBuilder(
        width=64,
        height=64,
        frame_rate=FrameRate(30, 1),
        output_path="particles.mp4",
        duration=1.0,
    )


def test_public_particle_authoring_serializes_schema_valid_source() -> None:
    project = builder()
    audio = project.add_audio_asset("examples/assets/tone.wav")
    project.audio.add_track(id="music").add_clip(asset=audio, start=0, trim_end=1.0)
    size = project.scalar_property(1.0)
    size.modulate(project.audio.master.rms(), mode="multiply")
    particle = ParticleSystem(
        seed=9,
        emitter=RectangleEmitter(Point(0.5, 0.0), Point(1.0, 0.1)),
        rate=10,
        bursts=(ParticleBurst(0.25, 3),),
        lifetime_range=ScalarRange(0.8, 1.2),
        size_range=ScalarRange(0.01, 0.02),
        direction=90,
        spread=12,
        primitive=ParticlePrimitive.SQUARE,
        blend_mode=ParticleBlendMode.ADDITIVE,
        lifetime_style=ParticleLifetimeStyle(
            opacity=(ScalarLifetimeStop(0.0, 1.0), ScalarLifetimeStop(1.0, 0.0)),
            colour=(
                ColourLifetimeStop(0.0, "#ffffff"),
                ColourLifetimeStop(1.0, "#66aaff"),
            ),
        ),
        audio_reactive=ParticleAudioReactive(size=size),
    )
    project.add_particle_system_clip(
        particle_system=particle, start=0, duration=1, layer=1
    )
    data = project.to_dict()
    assert not list(VALIDATOR.iter_errors(data))
    assert data["visual"]["clips"][0]["source"]["type"] == "particle_system"  # type: ignore[index]
    assert project.validate().is_valid


def test_particle_presets_are_deterministic_schema_valid_and_customizable() -> None:
    preset_calls = (ambient_stars, snow, embers, sparks, radial_burst)
    for preset in preset_calls:
        first = preset(seed=42)
        second = preset(seed=42)
        assert first.to_canonical() == second.to_canonical()
        project = builder()
        project.add_particle_system_clip(
            particle_system=first, start=0, duration=1, layer=1
        )
        assert not list(VALIDATOR.iter_errors(project.to_dict()))
        assert first.to_canonical()["particle"]["primitive"] in {"disc", "square"}  # type: ignore[index]
    assert sparks(seed=42).to_canonical() != sparks(seed=43).to_canonical()


@pytest.mark.parametrize(
    ("field", "value"),
    (("direction", math.nan), ("rotation", math.inf), ("angular_velocity", True)),
)
def test_particle_numeric_inputs_reject_non_finite_and_bool(
    field: str, value: object
) -> None:
    error = TypeError if value is True else ValueError
    with pytest.raises(error):
        ParticleSystem(**{field: value})  # type: ignore[arg-type]


@pytest.mark.parametrize(
    ("factory", "argument"),
    (
        ((PointEmitter, "position"), object()),
        ((RectangleEmitter, "center"), object()),
        ((RectangleEmitter, "size"), object()),
        ((CircleEmitter, "center"), object()),
    ),
)
def test_particle_emitters_reject_invalid_points(
    factory: tuple[type[object], str], argument: object
) -> None:
    emitter_type, field = factory
    with pytest.raises(TypeError):
        if emitter_type is PointEmitter:
            PointEmitter(position=argument)  # type: ignore[arg-type]
        elif emitter_type is RectangleEmitter:
            RectangleEmitter(
                argument if field == "center" else Point(0.5, 0.5),
                argument if field == "size" else Point(1, 1),
            )  # type: ignore[arg-type]
        else:
            CircleEmitter(argument)  # type: ignore[arg-type]


@pytest.mark.parametrize(
    ("field", "value"),
    (
        ("initial_velocity", object()),
        ("acceleration", object()),
        ("bursts", (object(),)),
        ("lifetime_style", object()),
        ("audio_reactive", object()),
        ("colour", "not-a-colour"),
    ),
)
def test_particle_public_objects_reject_invalid_values(
    field: str, value: object
) -> None:
    with pytest.raises((TypeError, ValueError)):
        ParticleSystem(**{field: value})  # type: ignore[arg-type]


def test_checked_in_particle_examples_are_schema_and_native_valid() -> None:
    for path in sorted((ROOT / "examples/particles").glob("*.json")):
        data = json.loads(path.read_text())
        assert not list(VALIDATOR.iter_errors(data)), path
        project = vestra.Project.from_dict(data, base_directory=ROOT)
        assert vestra.Editor().validate(project).is_valid, path


def test_audio_reactive_python_example_authors_valid_audio_material() -> None:
    namespace = runpy.run_path(str(ROOT / "examples/python/10_particles.py"))
    data = namespace["project"].to_dict()
    assert not list(VALIDATOR.iter_errors(data))
    assert data["assets"] == [
        {"id": "audio-000001", "type": "audio", "source": "examples/assets/tone.wav"}
    ]
    assert data["audio"]["tracks"][0]["clips"][0]["asset"] == "audio-000001"  # type: ignore[index]

    without_audio = json.loads(
        (ROOT / "examples/particles/audio-appearance.json").read_text()
    )
    without_audio["assets"] = []
    del without_audio["audio"]
    invalid = vestra.Project.from_dict(without_audio, base_directory=ROOT)
    assert not vestra.Editor().validate(invalid).is_valid


def test_particle_emitters_include_exact_circle_and_point_semantics() -> None:
    assert (
        CircleEmitter(Point(0.5, 0.5), 0.2, 0.2).to_canonical()["inner_radius"] == 0.2
    )
    assert PointEmitter().to_canonical()["position"] == {"x": 0.5, "y": 0.5}


def test_particle_authoring_renders_deterministically_on_cpu(tmp_path: Path) -> None:
    authored = builder()
    authored.base_directory = tmp_path
    authored.add_particle_system_clip(
        particle_system=sparks(count=24), start=0, duration=1, layer=1
    )
    prepared = vestra.Editor().prepare(
        authored.build(),
        vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    first = prepared.render_frame_number(5).to_bytes()
    second = prepared.render_frame_number(5).to_bytes()
    assert first == second
    assert any(channel != 0 for channel in first)
