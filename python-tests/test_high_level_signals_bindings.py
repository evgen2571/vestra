from __future__ import annotations

import pytest

import vestra
from vestra.authoring import (
    ParticleAudioReactive as AuthoringParticleAudioReactive,
    ParticleSystem as AuthoringParticleSystem,
    ProjectBuilder,
)
from vestra.properties import BindableScalarProperty, SignalBinding
from vestra.sources import Color, ParticleAudioReactive, ParticleSystem


def test_project_audio_signal_factory_is_stable_and_reuses_authoring_signal() -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    assert project.audio.signal is project.audio.signal
    signal = project.audio.signal.band_energy(40, 160).gain(2).clamp(0, 1)
    assert signal.to_canonical()["source"]["feature"]["type"] == "band_energy"  # type: ignore[index]
    assert [item["type"] for item in signal.to_canonical()["transforms"]] == [
        "gain",
        "clamp",
    ]  # type: ignore[index]


def test_all_signal_factories_and_transforms_keep_authored_order() -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    signals = project.audio.signal
    transformed = (
        signals.band_energy(40, 160)
        .gain(1.8)
        .envelope(0.025, 0.18)
        .remap(input=(0, 1), output=(0.95, 1.08))
        .clamp(0.5, 1.5)
        .response_curve(0.2, 0, 0.8, 1)
    )
    assert signals.rms().to_canonical()["source"]["feature"] == {"type": "rms"}  # type: ignore[index]
    assert signals.peak().to_canonical()["source"]["feature"] == {"type": "peak"}  # type: ignore[index]
    canonical = transformed.to_canonical()
    assert canonical["source"]["feature"] == {  # type: ignore[index]
        "type": "band_energy",
        "min_hz": 40.0,
        "max_hz": 160.0,
    }
    assert [item["type"] for item in canonical["transforms"]] == [  # type: ignore[index]
        "gain",
        "envelope",
        "remap",
        "clamp",
        "response_curve",
    ]


@pytest.mark.parametrize(
    "call",
    [
        lambda signal: signal.remap(input=(0, 1), output=None),
        lambda signal: signal.remap(0, 1, 2, 3, input=(0, 1), output=(0, 1)),
        lambda signal: signal.remap(input=(1, 0), output=(0, 1)),
        lambda signal: signal.envelope(-1, 0),
        lambda signal: signal.response_curve(0.8, 0, 0.2, 1),
    ],
)
def test_invalid_signal_transforms_do_not_mutate_the_original(call: object) -> None:
    signal = vestra.MasterAudioSignals().rms()
    before = signal.to_canonical()
    with pytest.raises((TypeError, ValueError)):
        call(signal)  # type: ignore[operator]
    assert signal.to_canonical() == before


def test_bindings_lower_in_order_and_uniform_scale_targets_both_components() -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    layer = project.root.add(Color("#ffffff"), id="layer")
    first = project.audio.signal.rms()
    second = project.audio.signal.peak()
    layer.opacity.bind(first, operation="replace").bind(second, operation="add")
    layer.transform.rotation.bind(first)
    layer.transform.scale.bind(second, operation="multiply")
    clip = project.snapshot().to_dict()["visual"]["clips"][0]  # type: ignore[index]
    assert [item["operation"] for item in clip["opacity"]["modifiers"]] == [
        "replace",
        "add",
    ]  # type: ignore[index]
    assert (
        clip["transform"]["component_modifiers"]["scale_x"]
        == clip["transform"]["component_modifiers"]["scale_y"]
    )  # type: ignore[index]


def test_transform_component_bindings_lower_and_copy_without_value_api() -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    layer = project.root.add(Color("#ffffff"), id="layer")
    signals = project.audio.signal

    layer.transform.position_x.bind(signals.rms(), operation="replace")
    layer.transform.position_y.bind(signals.peak(), operation="add")
    layer.transform.scale_x.bind(signals.rms(), operation="multiply")
    layer.transform.scale_y.bind(signals.peak(), operation="replace")

    assert not hasattr(layer.transform.position_x, "value")
    assert not hasattr(layer.transform.position_x, "keyframe")
    clip = project.snapshot().to_dict()["visual"]["clips"][0]  # type: ignore[index]
    modifiers = clip["transform"]["component_modifiers"]  # type: ignore[index]
    assert [item["operation"] for item in modifiers["position_x"]] == ["replace"]
    assert [item["operation"] for item in modifiers["position_y"]] == ["add"]
    assert [item["operation"] for item in modifiers["scale_x"]] == ["multiply"]
    assert [item["operation"] for item in modifiers["scale_y"]] == ["replace"]

    copied = layer.transform.copy()
    copied.position_x.clear_bindings()
    assert len(layer.transform.position_x.bindings) == 1
    assert copied.position_x.bindings == ()


def test_bindings_coexist_with_base_values_and_keyframes() -> None:
    project = vestra.Project(size=(2, 2), fps=2, duration=1)
    layer = project.root.add(Color("#ffffff"), id="layer")
    layer.opacity = 0.25
    layer.opacity.keyframe(0.5, 0.75)
    layer.opacity.bind(project.audio.signal.rms(), operation="multiply")
    opacity = project.snapshot().to_dict()["visual"]["clips"][0]["opacity"]  # type: ignore[index]
    assert opacity["base_value"] == 0.25
    assert opacity["keyframes"][0]["value"] == 0.75
    assert opacity["modifiers"][0]["operation"] == "multiply"


def test_whole_rotation_property_assignment_replaces_old_bindings() -> None:
    project = vestra.Project(size=(2, 2), fps=2, duration=1)
    layer = project.root.add(Color("#ffffff"))
    layer.transform.rotation.bind(project.audio.signal.rms(), operation="multiply")

    layer.transform.rotation = vestra.ScalarProperty(15)

    assert layer.transform.rotation.value == 15
    assert layer.transform.rotation.bindings == ()


def test_plain_rotation_assignment_only_updates_the_base_value() -> None:
    project = vestra.Project(size=(2, 2), fps=2, duration=1)
    layer = project.root.add(Color("#ffffff"))
    layer.transform.rotation.bind(project.audio.signal.rms(), operation="multiply")

    layer.transform.rotation = 15

    assert layer.transform.rotation.value == 15
    assert len(layer.transform.rotation.bindings) == 1


def test_invalid_binding_is_atomic_and_animation_only_properties_have_no_bind() -> None:
    prop = BindableScalarProperty(1)
    signal = vestra.ScalarSignal({"type": "rms"})
    with pytest.raises(TypeError):
        prop.bind(object())  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        prop.bind(signal, operation="bad")  # type: ignore[arg-type]
    assert prop.bindings == ()
    assert isinstance(prop.bind(signal), BindableScalarProperty)
    assert isinstance(prop.bindings[0], SignalBinding)


def test_particle_reactive_values_are_owner_free_and_lower_to_fresh_tracks() -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    size = BindableScalarProperty(1.0).bind(
        project.audio.signal.rms(), operation="multiply"
    )
    source = ParticleSystem(audio_reactive=ParticleAudioReactive(size=size))
    first = project.root.add(source, id="first")
    second = project.root.add(source, id="second")
    assert first.source.audio_reactive is not second.source.audio_reactive
    data = project.snapshot().to_dict()["visual"]["clips"]  # type: ignore[index]
    assert (
        data[0]["source"]["particle"]["audio_reactive"]["size"]["modifiers"][0][
            "operation"
        ]
        == "multiply"
    )  # type: ignore[index]


def test_particle_reactivity_converts_advanced_tracks_and_snapshots_repeatably() -> (
    None
):
    builder = ProjectBuilder(
        width=2,
        height=2,
        frame_rate=vestra.FrameRate(1, 1),
        output_path="out.mp4",
    )
    size = builder.scalar_property(1)
    size.keyframe(time=0.5, value=2)
    size.modulate(builder.audio.master.rms(), mode="multiply")
    advanced = AuthoringParticleSystem(
        audio_reactive=AuthoringParticleAudioReactive(size=size),
    )
    source = ParticleSystem(definition=advanced)
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    first = project.root.add(source, id="first")
    second = project.root.add(source, id="second")
    assert first.source.audio_reactive is not second.source.audio_reactive
    assert first.source.audio_reactive is not None
    assert first.source.audio_reactive.size is not None
    first.source.audio_reactive.size.value = 3
    assert second.source.audio_reactive is not None
    assert second.source.audio_reactive.size is not None
    assert second.source.audio_reactive.size.value == 1
    first_snapshot = project.snapshot().to_dict()
    assert project.snapshot().to_dict() == first_snapshot


def test_signal_transform_uses_adapter_only_when_native_source_needs_it() -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    plain = project.root.add(Color("#ffffff"), id="plain")
    assert (
        project.snapshot().to_dict()["visual"]["clips"][0]["source"]["type"]
        == "solid_color"
    )  # type: ignore[index]
    plain.transform.scale.bind(project.audio.signal.rms())
    adapted = project.snapshot().to_dict()["visual"]["clips"][0]  # type: ignore[index]
    assert adapted["source"]["type"] == "group"
    assert adapted["id"] == adapted["source"]["clips"][0]["id"] == "plain"


def test_nested_signal_binding_keeps_scoped_ids() -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    group = project.root.group(id="outer")
    layer = group.add(Color("#ffffff"), id="child")
    layer.transform.rotation.bind(project.audio.signal.peak(), operation="replace")
    child = project.snapshot().to_dict()["visual"]["clips"][0]["source"]["clips"][0]  # type: ignore[index]
    assert child["id"] == "outer/child"
    assert child["source"]["clips"][0]["id"] == "outer/child"


def test_cpu_render_evaluates_high_level_audio_signal_binding() -> None:
    project = vestra.Project(size=(8, 8), fps=4, duration=1, base_directory=".")
    layer = project.root.add(Color("#ff0000"), id="red")
    layer.opacity.bind(project.audio.signal.rms(), operation="replace")
    clip = project.audio.track("tone").add("examples/assets/tone.wav", trim_end=1)
    clip.set_gain_automation(
        [
            vestra.AudioGainKeyframe(0, 0),
            vestra.AudioGainKeyframe(0.5, 1),
        ]
    )

    quiet = project.render_frame(0, backend="cpu").to_bytes()[:4]
    loud = project.render_frame(0.75, backend="cpu").to_bytes()[:4]
    assert quiet != loud
    assert quiet[0] < 255 and loud[0] < 255
