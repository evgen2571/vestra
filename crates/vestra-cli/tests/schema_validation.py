#!/usr/bin/env python3
"""Validate the canonical project schema against representative inputs."""

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[3]
schema = json.loads((ROOT / "schemas/project.schema.json").read_text())
Draft202012Validator.check_schema(schema)
project = json.loads((ROOT / "examples/projects/animation-effects.json").read_text())
validator = Draft202012Validator(schema)


def errors(instance):
    return list(validator.iter_errors(instance))


assert not errors(project), "canonical example must validate"

for example in sorted((ROOT / "examples").rglob("*.json")):
    instance = json.loads(example.read_text())
    assert not errors(instance), f"example must validate: {example.relative_to(ROOT)}"
    output = instance["output"]
    frame_rate = output["frame_rate"]
    if isinstance(frame_rate, str):
        numerator, denominator = map(int, frame_rate.split("/"))
        frame_rate = numerator / denominator
    duration = output.get("duration")
    if duration is None:
        duration = max(
            (clip["start"] + clip["duration"] for clip in instance["visual"]["clips"]),
            default=0,
        )
    if example.parent.name in {"effects", "transitions", "presets", "compositing"}:
        assert frame_rate * duration >= 90, f"preview is too short: {example.relative_to(ROOT)}"
    for transition in instance["visual"].get("transitions", []):
        clips = {clip["id"]: clip for clip in instance["visual"]["clips"]}
        outgoing = clips[transition["outgoing"]]
        incoming = clips[transition["incoming"]]
        assert transition["start"] - outgoing["start"] >= 1, f"no outgoing lead-in: {example.relative_to(ROOT)}"
        assert incoming["start"] + incoming["duration"] - (transition["start"] + transition["duration"]) >= 1, f"no incoming settle: {example.relative_to(ROOT)}"

solid_colour = copy.deepcopy(project)
solid_colour["assets"] = []
solid_colour["visual"]["clips"] = [
    {
        "id": "canvas-colour",
        "source": {"type": "solid_color", "colour": "#112233"},
        "start": 0,
        "duration": 0.1,
        "layer": 0,
        "opacity": {"base_value": 1},
    }
]
solid_colour["visual"]["transitions"] = []
assert not errors(solid_colour), "solid colours do not need transforms"

solid_with_transform = copy.deepcopy(solid_colour)
solid_with_transform["visual"]["clips"][0]["transform"] = project["visual"]["clips"][0]["transform"]
assert errors(solid_with_transform), "solid colours must not accept transforms"

spectrum = copy.deepcopy(solid_colour)
spectrum["visual"]["clips"][0]["source"] = {"type": "spectrum2d"}
assert not errors(spectrum), "minimal Spectrum2D source must validate"
spectrum_explicit = copy.deepcopy(spectrum)
spectrum_explicit["visual"]["clips"][0]["source"] = {
    "type": "spectrum2d", "band_count": 32, "min_hz": 60, "max_hz": 12000,
    "sensitivity": 10, "attack_seconds": 0.04, "release_seconds": 0.3,
    "x": 0.05, "y": 0.65, "width": 0.9, "height": 0.3,
    "bar_gap_ratio": 0.4, "colour": "#12ab34cc",
}
assert not errors(spectrum_explicit), "explicit Spectrum2D source must validate"
for field, value in [
    ("band_count", 0), ("band_count", 49), ("min_hz", 0), ("max_hz", 0),
    ("max_hz", 24000.001), ("bar_gap_ratio", 1), ("width", 0), ("attack_seconds", -1),
]:
    invalid = copy.deepcopy(spectrum_explicit)
    invalid["visual"]["clips"][0]["source"][field] = value
    assert errors(invalid), f"invalid Spectrum2D {field} must not validate"
nyquist = copy.deepcopy(spectrum_explicit)
nyquist["visual"]["clips"][0]["source"]["max_hz"] = 24000
assert not errors(nyquist), "Spectrum2D Nyquist boundary must validate"
invalid_colour = copy.deepcopy(spectrum_explicit)
invalid_colour["visual"]["clips"][0]["source"]["colour"] = "not-a-colour"
assert errors(invalid_colour), "invalid Spectrum2D colour must not validate"

particle = copy.deepcopy(solid_colour)
particle["visual"]["clips"][0]["source"] = {
    "type": "particle_system",
    "seed": 7,
    "emitter": {"type": "point", "position": {"x": 0.5, "y": 0.5}},
    "emission": {"rate": 2.5, "bursts": [{"time": 0, "count": 2}]},
    "particle": {"lifetime": 1, "initial_velocity": {"x": 0, "y": 0},
                  "acceleration": {"x": 0, "y": 0}, "size": 1, "opacity": 1,
                  "colour": "#ffffff"},
}
assert not errors(particle), "ParticleSystem source must validate"
particle_explicit = copy.deepcopy(particle)
particle_explicit["visual"]["clips"][0]["source"]["particle"].update({
    "primitive": "square",
    "blend_mode": "additive",
})
assert not errors(particle_explicit), "explicit particle primitive and blend mode must validate"
expanded_particle = copy.deepcopy(particle)
expanded_particle["visual"]["clips"][0]["source"].update({
    "emitter": {"type": "circle", "center": {"x": 0.5, "y": 0.5},
                "inner_radius": 0.1, "outer_radius": 0.3},
    "particle": {
        "lifetime": 2, "lifetime_range": {"min": 1, "max": 3},
        "size": 0.02, "size_range": {"min": 0.01, "max": 0.04},
        "speed": 0.2, "speed_range": {"min": 0.1, "max": 0.4},
        "direction_degrees": 90, "direction_spread_degrees": 30,
        "rotation_range": {"min": -180, "max": 180},
        "angular_velocity_range": {"min": -90, "max": 90},
        "colour": "#ffffff",
    },
})
assert not errors(expanded_particle), "expanded particle fields must validate"
styled_particle = copy.deepcopy(particle)
styled_particle["visual"]["clips"][0]["source"]["particle"].update({
    "lifetime_style": {
        "size": [{"t": 0.0, "value": 0.0}, {"t": 1.0, "value": 1.0}],
        "opacity": [{"t": 0.0, "value": 1.0}, {"t": 1.0, "value": 0.0}],
        "colour": [{"t": 0.0, "colour": "#ffffff"}, {"t": 1.0, "colour": "#ff0000"}],
    },
    "audio_reactive": {
        "size": {
            "base_value": 1.0,
            "modifiers": [{"operation": "multiply", "signal": {
                "source": {"type": "audio", "tap": "master", "feature": {"type": "rms"}}
            }}],
        },
    },
})
assert not errors(styled_particle), "particle lifetime and audio appearance styling must validate"
invalid_lifetime_stop = copy.deepcopy(styled_particle)
invalid_lifetime_stop["visual"]["clips"][0]["source"]["particle"]["lifetime_style"]["size"][0]["t"] = 2
assert errors(invalid_lifetime_stop), "out-of-range particle lifetime stop must not validate"
rectangle_particle = copy.deepcopy(expanded_particle)
rectangle_particle["visual"]["clips"][0]["source"]["emitter"] = {
    "type": "rectangle", "center": {"x": 0.5, "y": 0.5},
    "size": {"x": 1, "y": 0},
}
assert not errors(rectangle_particle), "rectangle emitter must validate"
for invalid_range in [{"min": "bad", "max": 1}, {"min": 3}]:
    invalid_particle = copy.deepcopy(expanded_particle)
    invalid_particle["visual"]["clips"][0]["source"]["particle"]["size_range"] = invalid_range
    assert errors(invalid_particle), "invalid particle range must not validate"
invalid_circle = copy.deepcopy(expanded_particle)
invalid_circle["visual"]["clips"][0]["source"]["emitter"]["outer_radius"] = -1
assert errors(invalid_circle), "negative circle radius must not validate"
for field, invalid in [("primitive", "triangle"), ("blend_mode", "screen")]:
    invalid_particle = copy.deepcopy(particle_explicit)
    invalid_particle["visual"]["clips"][0]["source"]["particle"][field] = invalid
    assert errors(invalid_particle), f"invalid particle {field} must not validate"
particle_with_transform = copy.deepcopy(particle)
particle_with_transform["visual"]["clips"][0]["transform"] = project["visual"]["clips"][0]["transform"]
assert errors(particle_with_transform), "ParticleSystem transforms must remain unsupported"

wrong_position = copy.deepcopy(project)
wrong_position["visual"]["clips"][0]["transform"]["position"]["base_value"] = 1
assert errors(wrong_position), "scalar position must not validate"

wrong_opacity = copy.deepcopy(project)
wrong_opacity["visual"]["clips"][0]["opacity"]["base_value"] = 2
assert errors(wrong_opacity), "out-of-range opacity must not validate"

zero_scale = copy.deepcopy(project)
zero_scale["visual"]["clips"][0]["transform"]["scale"]["base_value"]["x"] = 0
assert errors(zero_scale), "zero scale must not validate"

invalid_anchor = copy.deepcopy(project)
invalid_anchor["visual"]["clips"][0]["transform"]["anchor"]["base_value"]["x"] = -0.1
assert errors(invalid_anchor), "out-of-range anchor must not validate"

invalid_tint = copy.deepcopy(project)
invalid_tint["visual"]["clips"][0]["effects"] = [
    {"id": "tint", "type": "tint", "colour": "#abcdef", "amount": {"base_value": 2}}
]
assert errors(invalid_tint), "out-of-range tint amount must not validate"

invalid_glow_keyframe = copy.deepcopy(solid_colour)
invalid_glow_keyframe["visual"]["clips"][0]["effects"] = [{
    "id": "glow",
    "type": "glow",
    "threshold": {"base_value": 0.5},
    "radius": {"base_value": 4, "keyframes": [{"time": 1, "value": 100, "interpolation": "linear"}]},
    "intensity": {"base_value": 1},
    "colour": "#ffffff",
}]
assert errors(invalid_glow_keyframe), "out-of-range glow radius keyframe must not validate"

valid_glow_boundary_keyframe = copy.deepcopy(invalid_glow_keyframe)
valid_glow_boundary_keyframe["visual"]["clips"][0]["effects"][0]["radius"]["keyframes"][0]["value"] = 32
assert not errors(valid_glow_boundary_keyframe), "inclusive glow radius boundary must validate"

invalid_gamma_base = copy.deepcopy(solid_colour)
invalid_gamma_base["visual"]["clips"][0]["effects"] = [{
    "id": "color-adjust",
    "type": "color_adjust",
    "exposure": {"base_value": 0},
    "gamma": {"base_value": 0},
    "black_point": {"base_value": 0},
    "white_point": {"base_value": 1},
}]
assert errors(invalid_gamma_base), "exclusive gamma minimum must not validate"

invalid_gamma_keyframe = copy.deepcopy(invalid_gamma_base)
invalid_gamma_keyframe["visual"]["clips"][0]["effects"][0]["gamma"] = {
    "base_value": 1, "keyframes": [{"time": 1, "value": 0, "interpolation": "linear"}],
}
assert errors(invalid_gamma_keyframe), "exclusive gamma keyframe minimum must not validate"

valid_black_point_boundary = copy.deepcopy(solid_colour)
valid_black_point_boundary["visual"]["clips"][0]["effects"] = [{
    "id": "color-adjust",
    "type": "color_adjust",
    "exposure": {"base_value": 0},
    "gamma": {"base_value": 1},
    "black_point": {"base_value": 0.999},
    "white_point": {"base_value": 1},
}]
assert not errors(valid_black_point_boundary), "near-exclusive black point boundary must validate"

invalid_black_point_base = copy.deepcopy(valid_black_point_boundary)
invalid_black_point_base["visual"]["clips"][0]["effects"][0]["black_point"]["base_value"] = 1
assert errors(invalid_black_point_base), "exclusive black point maximum must not validate"

invalid_black_point_keyframe = copy.deepcopy(valid_black_point_boundary)
invalid_black_point_keyframe["visual"]["clips"][0]["effects"][0]["black_point"] = {
    "base_value": 0, "keyframes": [{"time": 1, "value": 1, "interpolation": "linear"}],
}
assert errors(invalid_black_point_keyframe), "exclusive black point keyframe maximum must not validate"

out_of_range_shake_seed = copy.deepcopy(solid_colour)
out_of_range_shake_seed["visual"]["clips"][0]["effects"] = [{
    "id": "shake",
    "type": "camera_shake",
    "position_amount": {"base_value": 0},
    "rotation_degrees": {"base_value": 0},
    "scale_amount": {"base_value": 0},
    "frequency": {"base_value": 1},
    "seed": 2**64,
    "attack": 0,
    "decay": 1,
}]
assert errors(out_of_range_shake_seed), "u64 camera-shake seed must not validate"

unknown_effect_field = copy.deepcopy(project)
unknown_effect_field["visual"]["clips"][0]["effects"][0]["unknown"] = True
assert errors(unknown_effect_field), "unknown effect fields must not validate"

unknown_transition_field = copy.deepcopy(project)
unknown_transition_field["visual"]["transitions"][0]["unknown"] = True
assert errors(unknown_transition_field), "unknown transition fields must not validate"

versioned = copy.deepcopy(project)
versioned["version"] = 2
assert errors(versioned), "project version fields must not validate"

for pointer in [("name",), ("metadata",), ("audio",), ("output", "duration"), ("visual", "clips", 0, "sizing"), ("visual", "clips", 0, "crop"), ("visual", "clips", 0, "transform")]:
    null_field = copy.deepcopy(project)
    target = null_field
    for key in pointer[:-1]:
        target = target[key]
    target[pointer[-1]] = None
    assert errors(null_field), f"explicit null at {pointer} must not validate"

null_audio_trim = copy.deepcopy(project)
null_audio_trim["audio"] = {"asset": "audio", "timeline_start": 0, "trim_start": 0, "trim_end": None, "volume": 1}
assert errors(null_audio_trim), "explicit null audio trim_end must not validate"

audio_timeline = copy.deepcopy(solid_colour)
audio_timeline["assets"] = [{"id": "audio", "type": "audio", "source": "tone.wav"}]
audio_timeline["audio"] = {"tracks": [{"id": "music", "gain": 1, "mute": False, "clips": [{"id": "clip", "asset": "audio", "start": 0, "trim_start": 0, "gain": 1, "fade_in": 0, "fade_out": 0, "mute": False}]}]}
assert not errors(audio_timeline), "schema-v2 audio timeline must validate"

audio_effect = {
    "id": "eq", "type": "parametric_eq", "frequency_hz": 120.0,
    "gain_db": 6.0, "q": 0.8,
}

playback_speed = {"id": "speed", "type": "playback_speed", "rate": 2.0}
clip_with_speed = copy.deepcopy(audio_timeline)
clip_with_speed["audio"]["tracks"][0]["clips"][0]["effects"] = [playback_speed]
assert not errors(clip_with_speed), "playback speed must validate at clip scope"
for scope in ("master", "track"):
    scoped = copy.deepcopy(audio_timeline)
    target = scoped["audio"] if scope == "master" else scoped["audio"]["tracks"][0]
    target["effects"] = [playback_speed]
    assert errors(scoped), f"playback speed must be rejected at {scope} scope"
for value in (0.25, 4.0):
    candidate = copy.deepcopy(clip_with_speed)
    candidate["audio"]["tracks"][0]["clips"][0]["effects"][0]["rate"] = value
    assert not errors(candidate), "playback speed boundary must validate"
for value in (0, 0.249, 4.001):
    candidate = copy.deepcopy(clip_with_speed)
    candidate["audio"]["tracks"][0]["clips"][0]["effects"][0]["rate"] = value
    assert errors(candidate), "playback speed out-of-range value must fail"

for scope in ("master", "track", "clip"):
    scoped = copy.deepcopy(audio_timeline)
    if scope == "master":
        scoped["audio"]["effects"] = [audio_effect]
    elif scope == "track":
        scoped["audio"]["tracks"][0]["effects"] = [audio_effect]
    else:
        scoped["audio"]["tracks"][0]["clips"][0]["effects"] = [audio_effect]
    assert not errors(scoped), f"audio effects must validate at {scope} scope"

def with_master_effect(effect):
    scoped = copy.deepcopy(audio_timeline)
    scoped["audio"]["effects"] = [effect]
    return scoped

for field, invalid, valid in [
    ("frequency_hz", [0, 24000.1], [24000]),
    ("gain_db", [-24.1, 24.1], [-24, 24]),
    ("q", [0, 100.1], [0.8, 100]),
]:
    for value in invalid:
        candidate = {**audio_effect, field: value}
        assert errors(with_master_effect(candidate)), f"invalid {field} boundary must fail"
    for value in valid:
        candidate = {**audio_effect, field: value}
        assert not errors(with_master_effect(candidate)), f"valid {field} boundary must pass"

assert errors(with_master_effect({**audio_effect, "made_up": 123})), "unknown audio effect fields must fail"

audio_reactive = copy.deepcopy(audio_timeline)
audio_reactive["visual"]["clips"][0]["opacity"]["modifiers"] = [{
    "operation": "add",
    "signal": {
        "source": {"type": "audio", "tap": "master", "feature": {"type": "band_energy", "min_hz": 40, "max_hz": 160}},
        "transforms": [
            {"type": "gain", "gain": 2},
            {"type": "remap", "input_min": 0, "input_max": 1, "output_start": 0, "output_end": 1},
            {"type": "clamp", "min": 0, "max": 1},
            {"type": "envelope", "attack": 0.02, "release": 0.18},
            {"type": "response_curve", "x1": 0.42, "y1": 0, "x2": 0.58, "y2": 1},
        ],
    },
}]
assert not errors(audio_reactive), "supported scalar properties must accept inline audio signals"

point_modifiers = copy.deepcopy(project)
point_modifiers["visual"]["clips"][0]["transform"]["position"]["modifiers"] = []
assert errors(point_modifiers), "point tracks must not expose scalar modifiers"

component_modifiers = copy.deepcopy(project)
component_modifiers["visual"]["clips"][0]["transform"]["component_modifiers"] = {
    "position_x": audio_reactive["visual"]["clips"][0]["opacity"]["modifiers"],
    "scale_y": audio_reactive["visual"]["clips"][0]["opacity"]["modifiers"],
}
assert not errors(component_modifiers), "transform component modifiers must be accepted separately from point tracks"

unsupported_softness = copy.deepcopy(audio_reactive)
unsupported_softness["visual"]["clips"][0]["effects"] = [{"id": "vignette", "type": "vignette", "amount": {"base_value": 0.5}, "radius": {"base_value": 1}, "softness": {"base_value": 0.2, "modifiers": []}, "colour": "#000000"}]
assert errors(unsupported_softness), "deferred scalar properties must not expose modifiers"

old_audio_shape = copy.deepcopy(audio_timeline)
old_audio_shape["audio"] = {"asset": "audio", "timeline_start": 0, "trim_start": 0, "volume": 1}
assert errors(old_audio_shape), "old global audio shape must not validate"

schema_v1 = copy.deepcopy(audio_timeline)
schema_v1["schema_version"] = 1
assert errors(schema_v1), "schema v1 must not validate"
