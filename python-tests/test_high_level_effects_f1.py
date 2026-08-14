from __future__ import annotations

import pytest

import vestra
from vestra.effects import (
    ActiveInterval,
    Bloom,
    Brightness,
    CameraShake,
    ColorAdjust,
    EffectStack,
    MotionBlur,
    Vignette,
    available_effects,
)
from vestra.authoring.values import Point


def _effect_values() -> dict[str, object]:
    return {
        "brightness": Brightness(0.1),
        "contrast": vestra.Contrast(1.0),
        "saturation": vestra.Saturation(1.0),
        "tint": vestra.Tint("#ffffff", 0.5),
        "gaussian_blur": vestra.GaussianBlur(1),
        "directional_blur": vestra.DirectionalBlur(1, 45),
        "zoom_blur": vestra.ZoomBlur(1, 2, Point(0.5, 0.5)),
        "glow": vestra.Glow(0.5, 1, 1, "#ffffff"),
        "bloom": Bloom(0.5, 1, 1),
        "chromatic_aberration": vestra.ChromaticAberration(1, 45),
        "vignette": Vignette(0.5, 1, 0.5, "#000000"),
        "sharpen": vestra.Sharpen(0.5, 1),
        "color_adjust": ColorAdjust(0, 1, 0.2, 0.8),
        "camera_shake": CameraShake(1, 1, 0.1, 1, 1, 0, 1),
        "motion_blur": MotionBlur(1, 180, 2, 2),
    }


def test_public_catalog_has_one_descriptor_for_each_native_effect() -> None:
    native = {str(item["id"]) for item in vestra.authoring.available_effects()}
    public = {str(item["id"]) for item in available_effects()}
    assert public == native
    assert len(public) == 15


def test_catalog_parameters_have_the_declared_public_property_kinds() -> None:
    effects = _effect_values()
    for definition in available_effects():
        effect = effects[str(definition["id"])]
        parameters = definition["parameters"]
        property_names = {
            str(item["name"])
            for item in parameters
            if item["kind"] in {"scalar_property", "plain_track"}
        }
        assert {name for name, _ in effect._property_items()} == property_names  # type: ignore[attr-defined]
        for parameter in parameters:
            if parameter["kind"] == "scalar_property":
                assert isinstance(
                    getattr(effect, str(parameter["name"])),
                    vestra.BindableScalarProperty,
                )
            elif parameter["kind"] == "plain_track":
                assert (
                    type(getattr(effect, str(parameter["name"])))
                    is vestra.ScalarProperty
                )
    assert not hasattr(effects["chromatic_aberration"], "radius")
    assert not hasattr(effects["sharpen"], "threshold")
    assert not hasattr(effects["sharpen"], "intensity")


def test_effect_constraints_and_setters_are_atomic() -> None:
    with pytest.raises(ValueError):
        ColorAdjust(0, 0, 0.2, 0.8)
    with pytest.raises(ValueError):
        Vignette(0.5, 1, 0, "#000000")
    effect = Bloom(0.5, 1, 1)
    before = effect.threshold.value
    with pytest.raises(ValueError):
        effect.threshold = vestra.ScalarProperty(2)
    assert effect.threshold.value == before
    with pytest.raises(TypeError):
        vestra.ZoomBlur(1, 2, Point(0.5, 0.5), direction=object())  # type: ignore[arg-type]


def test_every_numeric_effect_boundary_matches_native_metadata() -> None:
    effects = _effect_values()
    for definition in available_effects():
        effect = effects[str(definition["id"])]
        for parameter in definition["parameters"]:
            name = str(parameter["name"])
            if parameter["kind"] in {"scalar_property", "plain_track"}:
                prop = getattr(effect, name)
                original = prop.value
                for bound_name, direction in (("minimum", -1), ("maximum", 1)):
                    bound = parameter[bound_name]
                    if bound is None:
                        continue
                    exclusive = bool(parameter[f"{bound_name}_exclusive"])
                    if exclusive:
                        with pytest.raises(ValueError):
                            prop.value = bound
                    else:
                        prop.value = bound
                    with pytest.raises(ValueError):
                        prop.value = float(bound) + direction
                    prop.value = original
            elif parameter["kind"] in {"integer", "number"}:
                original = getattr(effect, name)
                minimum = (
                    parameter["integer_minimum"]
                    if parameter["kind"] == "integer"
                    else parameter["minimum"]
                )
                maximum = (
                    parameter["integer_maximum"]
                    if parameter["kind"] == "integer"
                    else parameter["maximum"]
                )
                if minimum is not None:
                    if parameter["minimum_exclusive"]:
                        with pytest.raises(ValueError):
                            setattr(effect, name, minimum)
                    else:
                        setattr(effect, name, minimum)
                    with pytest.raises(ValueError):
                        setattr(effect, name, minimum - 1)
                if maximum is not None:
                    if parameter["maximum_exclusive"]:
                        with pytest.raises(ValueError):
                            setattr(effect, name, maximum)
                    else:
                        setattr(effect, name, maximum)
                    with pytest.raises(ValueError):
                        setattr(effect, name, maximum + 1)
                setattr(effect, name, original)


def test_effect_tracks_bindings_lower_in_order_and_snapshot_isolation() -> None:
    project = vestra.Project(size=(4, 4), fps=4, duration=1)
    layer = project.root.add(vestra.sources.Color("#808080"), duration=1)
    effect = layer.effects.add(Bloom(0.5, 1, 1))
    signal = project.audio.signal.rms()
    effect.threshold.keyframe(0.25, 0.75)
    effect.threshold.bind(signal, operation="multiply")
    first = project.snapshot().to_dict()
    effect.threshold.keyframe(0.5, 1)
    second = project.snapshot().to_dict()
    first_track = first["visual"]["clips"][0]["effects"][0]["threshold"]  # type: ignore[index]
    second_track = second["visual"]["clips"][0]["effects"][0]["threshold"]  # type: ignore[index]
    assert len(first_track["keyframes"]) == 1  # type: ignore[index]
    assert len(second_track["keyframes"]) == 2  # type: ignore[index]
    assert second_track["modifiers"][0]["operation"] == "multiply"  # type: ignore[index]


def test_extend_order_ids_and_failed_extensions_are_atomic() -> None:
    stack = EffectStack("post")
    stack.add(Bloom(0.5, 1, 1, id="one"))
    before = stack.items
    with pytest.raises(ValueError):
        stack.extend(
            [
                Vignette(0.2, 1, 0.5, "#000000"),
                vestra.CameraShake(1, 1, 0.1, 1, 1, 0, 1),
            ]
        )
    assert stack.items == before
    stack.extend(
        [Vignette(0.2, 1, 0.5, "#000000", id="two"), Bloom(0.1, 1, 1, id="three")]
    )
    assert [item.id for item in stack.items] == ["one", "two", "three"]


def test_adapter_and_nested_layers_receive_outer_effects_and_cpu_render_changes_pixels() -> (
    None
):
    project = vestra.Project(size=(8, 8), fps=4, duration=1)
    group = project.root.group("nested", duration=1)
    nested = group.add(vestra.sources.Color("#202020"), duration=1)
    nested.effects.add(Brightness(1))
    spectrum = project.root.add(
        vestra.sources.Spectrum2D(band_count=8, min_hz=20, max_hz=20000), duration=1
    )
    spectrum.transform.scale = 2
    spectrum.effects.add(Bloom(0.5, 1, 1))
    data = project.snapshot().to_dict()
    group_data = data["visual"]["clips"][0]  # type: ignore[index]
    assert group_data["source"]["clips"][0]["effects"][0]["type"] == "brightness"  # type: ignore[index]
    assert data["visual"]["clips"][1]["effects"][0]["type"] == "bloom"  # type: ignore[index]
    plain = vestra.Project(size=(4, 4), fps=4, duration=1)
    plain.root.add(vestra.sources.Color("#202020"), duration=1)
    changed = vestra.Project(size=(4, 4), fps=4, duration=1)
    changed_layer = changed.root.add(vestra.sources.Color("#202020"), duration=1)
    changed_layer.effects.add(Brightness(1))
    assert (
        plain.render_frame(0, backend="cpu").to_bytes()
        != changed.render_frame(0, backend="cpu").to_bytes()
    )


def test_group_layer_effects_lower_after_child_precomposition() -> None:
    project = vestra.Project(size=(8, 8), fps=4, duration=1)
    group = project.root.group("nested", duration=1)
    group.add(vestra.sources.Color("#202020"), duration=1)
    group.effects.add(Brightness(1))

    group_data = project.snapshot().to_dict()["visual"]["clips"][0]
    assert group_data["effects"][0]["type"] == "brightness"


def test_effect_properties_are_stable_typed_and_validate_metadata() -> None:
    effect = Bloom(threshold=0.5, radius=4, intensity=1)
    assert effect.threshold is effect.threshold
    assert isinstance(effect.threshold, vestra.BindableScalarProperty)
    effect.threshold.keyframe(0.5, 0.75)
    effect.threshold.bind(vestra.ScalarSignal({"type": "rms"}))
    with pytest.raises(ValueError):
        Vignette(amount=0.5, radius=1, softness=0, colour="#ffffff")


def test_effect_stack_copies_on_add_and_rejects_clip_only_post_effects() -> None:
    first = EffectStack("layer")
    second = EffectStack("layer")
    source = Bloom(threshold=0.5, radius=4, intensity=1)
    source.threshold.keyframe(0.25, 0.75)
    source.threshold.bind(vestra.MasterAudioSignals().rms(), operation="multiply")
    added_first = first.add(source)
    added_second = second.add(source)
    added_first.threshold.value = 0.25
    assert added_second.threshold.value == 0.5
    assert source.threshold.value == 0.5
    assert (
        len(added_first.threshold.keyframes)
        == len(added_second.threshold.keyframes)
        == 1
    )
    assert added_first.threshold.bindings == added_second.threshold.bindings
    added_first.threshold = vestra.ScalarProperty(0.4)
    assert added_first.threshold.bindings == ()
    assert len(added_second.threshold.bindings) == 1
    with pytest.raises(ValueError):
        EffectStack("post").add(
            CameraShake(
                active_interval=ActiveInterval(),
                position_amount=1,
                rotation_degrees=1,
                scale_amount=0.1,
                frequency=2,
                seed=1,
                attack=0,
                decay=1,
            )
        )
    with pytest.raises(ValueError):
        EffectStack("post").add(MotionBlur(1, 180, 2, 2))


def test_spectrum_preset_effects_remain_inside_layer_adapter() -> None:
    project = vestra.Project(size=(8, 8), fps=4, duration=1)
    layer = project.root.add(vestra.sources.Spectrum2D(preset="neon"), duration=1)
    layer.transform.scale = 2
    layer.effects.add(Brightness(0.1))
    outer = project.snapshot().to_dict()["visual"]["clips"][0]
    inner = outer["source"]["clips"][0]
    assert [item["type"] for item in outer["effects"]] == ["brightness"]
    assert len(inner["effects"]) == 2
    assert project.snapshot().to_dict()["visual"]["clips"][0] == outer


def test_effects_lower_on_layers_and_root_post_effects() -> None:
    project = vestra.Project(size=(16, 16), fps=24, duration=1)
    layer = project.root.add(vestra.sources.Color("#ffffff"), duration=1)
    layer.effects.add(Bloom(threshold=0.5, radius=4, intensity=1))
    project.post_effects.add(
        Vignette(amount=0.2, radius=1, softness=1, colour="#000000")
    )
    data = project.snapshot().to_dict()
    assert data["visual"]["clips"][0]["effects"][0]["type"] == "bloom"  # type: ignore[index]
    assert data["visual"]["post_effects"][0]["type"] == "vignette"  # type: ignore[index]
