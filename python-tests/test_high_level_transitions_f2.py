"""High-level transitions, cinematic presets, and root flashes."""

from __future__ import annotations

import pytest

import vestra
from vestra.presets import Preset
from vestra.transitions import (
    Crossfade,
    DirectionalPush,
    FlashCut,
    ZoomBlur,
    ZoomCrossfade,
    Transition,
)
from vestra.effects import Brightness
from vestra.sources import Color, Image
from vestra.authoring import Sizing


def _project() -> vestra.Project:
    return vestra.Project(size=(8, 6), fps=10, duration=2)


def test_root_transition_descriptors_lower_in_authored_order() -> None:
    project = _project()
    first = project.root.add(Image("red.png"), id="first")
    second = project.root.add(Image("blue.png"), id="second")
    values = [
        Crossfade(start=0, duration=0.2),
        ZoomCrossfade(
            start=0.2, duration=0.2, outgoing_zoom=1.2, incoming_start_zoom=0.8
        ),
        FlashCut(start=0.4, duration=0.2, colour="#ffffff", intensity=0.5),
        DirectionalPush(
            start=0.6, duration=0.2, angle_degrees=90, distance=1, blur_radius=2
        ),
        ZoomBlur(
            start=0.8,
            duration=0.2,
            outgoing_zoom=1.2,
            incoming_start_zoom=0.8,
            blur_radius=2,
        ),
    ]
    for value in values:
        project.root.transitions.add(first, second, value)
    transitions = project.snapshot().to_dict()["visual"]["transitions"]
    assert [item["type"] for item in transitions] == [
        "crossfade",
        "zoom_crossfade",
        "flash_cut",
        "directional_push",
        "zoom_blur",
    ]


def test_transition_endpoint_adapts_color_once_and_rejects_nested_composition() -> None:
    project = _project()
    first = project.root.add(Color("#ff0000"), id="first")
    second = project.root.add(Image("blue.png"), id="second")
    project.root.transitions.add(first, second, Crossfade(start=0, duration=1))
    outer = project.snapshot().to_dict()["visual"]["clips"][0]
    assert outer["source"]["type"] == "group"
    assert outer["source"]["clips"][0]["source"]["type"] == "solid_color"
    nested = project.root.group("nested")
    with pytest.raises(ValueError, match="root composition"):
        nested.child.transitions.add(second, first, Crossfade(start=0, duration=1))


def test_layer_preset_is_image_only_and_flashes_are_root_owned() -> None:
    project = _project()
    image = project.root.add(Image("red.png"), id="image")
    image.presets.apply_impact(seed=7, duration=0.4)
    assert image.presets.current == Preset("impact", duration=0.4, seed=7)
    with pytest.raises(TypeError, match="Image"):
        project.root.add(Color("#ff0000")).presets.apply_slow_drift()
    flash = project.flashes.add(
        start=0.5, duration=0.2, colour="#ffffff", fade_in=0.1, fade_out=0.1
    )
    assert flash.id == "flash-000001"
    assert project.snapshot().to_dict()["visual"]["flashes"][0]["fade_in"] == 0.1


@pytest.mark.parametrize(
    ("method", "kwargs", "kind"),
    [
        ("apply_slow_drift", {}, "slow_drift"),
        (
            "apply_zoom_punch",
            {"intensity": 0.5, "start": 0.1, "duration": 0.2},
            "zoom_punch",
        ),
        ("apply_impact", {"seed": 7}, "impact"),
        ("apply_heavy_impact", {"seed": 8}, "heavy_impact"),
        ("apply_focus_reveal", {}, "focus_reveal"),
    ],
)
def test_all_cinematic_presets_keep_native_canonical_semantics(
    method: str, kwargs: dict[str, object], kind: str
) -> None:
    project = _project()
    layer = project.root.add(Image("image.png"))
    preset = getattr(layer.presets, method)(**kwargs)
    assert preset.kind == kind
    native_preset = project.snapshot().to_dict()["visual"]["clips"][0]["preset"]
    expected = {**preset.to_canonical(), "start": preset.start}
    assert native_preset == expected
    before = project.snapshot().to_dict()
    with pytest.raises(ValueError, match="already has"):
        layer.presets.apply_slow_drift()
    assert project.snapshot().to_dict() == before
    layer.presets.clear()
    assert layer.presets.current is None


def test_transition_ownership_and_failure_atomicity() -> None:
    project = _project()
    first = project.root.add(Image("first.png"))
    second = project.root.add(Image("second.png"))
    other = _project().root.add(Image("other.png"))
    with pytest.raises(ValueError, match="different"):
        project.root.transitions.add(first, first, Crossfade(0, 1))
    with pytest.raises(ValueError, match="same composition"):
        project.root.transitions.add(first, other, Crossfade(0, 1))
    with pytest.raises(TypeError, match="supported"):
        project.root.transitions.add(first, second, Transition(0, 1))
    assert project.root.transitions.items == ()
    first_transition = project.root.transitions.add(
        first, second, Crossfade(0, 1), id="chosen"
    )
    assert first_transition.id == "chosen"
    with pytest.raises(ValueError, match="duplicate"):
        project.root.transitions.add(first, second, Crossfade(1, 1), id="chosen")
    assert project.root.transitions.items == (first_transition,)


def test_transition_rejects_an_unattached_layer_with_a_matching_composition() -> None:
    project = _project()
    attached = project.root.add(Image("attached.png"))
    orphan = vestra.Layer(
        project.root,
        Image("orphan.png"),
        identifier="orphan",
        name=None,
        start=0,
        duration=1,
        z=0,
        visible=True,
        opacity=1,
    )

    with pytest.raises(ValueError, match="owned by this composition"):
        project.root.transitions.add(orphan, attached, Crossfade(0, 1))

    assert project.root.transitions.items == ()


def test_transition_adapter_is_minimal_and_keeps_effects_on_the_final_clip() -> None:
    project = _project()
    endpoint = project.root.add(Color("#ff0000"), id="endpoint")
    endpoint.transform.scale = 2
    endpoint.effects.add(Brightness(0.1))
    direct = project.root.add(Image("image.png"), id="direct")
    project.root.add(Color("#00ff00"), id="plain")
    project.root.transitions.add(endpoint, direct, Crossfade(0, 1))
    clips = project.snapshot().to_dict()["visual"]["clips"]
    adapted = clips[0]
    assert adapted["source"]["type"] == "group"
    assert adapted["source"]["clips"][0]["source"]["type"] == "solid_color"
    assert [effect["type"] for effect in adapted["effects"]] == ["brightness"]
    assert clips[1]["source"]["type"] == "image"
    assert clips[2]["source"]["type"] == "solid_color"


def test_transition_preserves_native_fit_conflict_and_hidden_diagnostics() -> None:
    project = _project()
    outgoing = project.root.add(Image("first.png"), start=0, duration=1)
    incoming = project.root.add(Image("second.png"), start=0, duration=1, visible=False)
    project.root.transitions.add(
        outgoing, incoming, Crossfade(start=0.75, duration=0.5)
    )
    codes = {item.code for item in project.validate().diagnostics}
    assert "MVP-TRANSITION-HIDDEN" in codes
    assert "MVP-TRANSITION-FIT" in codes

    incoming.visible = True
    incoming.duration = 2
    third = project.root.add(Image("third.png"), duration=2)
    project.root.transitions.add(incoming, third, Crossfade(start=0.8, duration=0.5))
    assert "MVP-TRANSITION-CONFLICT" in {
        item.code for item in project.validate().diagnostics
    }


def test_flash_copy_on_add_setters_and_snapshots_are_atomic() -> None:
    project = _project()
    template = vestra.Flash(0.5, 0.4, "#ffffff", fade_in=0.1, fade_out=0.1)
    owned = project.flashes.add(template)
    template.opacity = 0.25
    assert owned.opacity == 1.0
    with pytest.raises(TypeError, match="either"):
        project.flashes.add(template, opacity=0.5)  # type: ignore[call-overload]
    before = owned.to_canonical()
    with pytest.raises(ValueError):
        owned.duration = 0.1
    with pytest.raises(ValueError):
        owned.fade_in = 0.4
    assert owned.to_canonical() == before
    first = project.snapshot().to_dict()
    assert project.snapshot().to_dict() == first


def test_high_level_flash_fade_changes_cpu_pixels() -> None:
    project = vestra.Project(size=(4, 4), fps=10, duration=1, background="#808080")
    project.flashes.add(
        start=0.1,
        duration=0.6,
        colour="#ff0000",
        opacity=0.5,
        fade_in=0.2,
        fade_out=0.2,
    )
    before = project.render_frame(0, backend="cpu").to_bytes()
    fade = project.render_frame(0.2, backend="cpu").to_bytes()
    peak = project.render_frame(0.4, backend="cpu").to_bytes()
    assert peak[0] > fade[0] > before[0]
    assert peak[1] < fade[1] < before[1]


@pytest.mark.parametrize(
    ("transition", "expected"),
    [
        (Crossfade(start=0.5, duration=1), (63, 1, 128, 255)),
        (
            ZoomCrossfade(
                start=0.5, duration=1, outgoing_zoom=1.2, incoming_start_zoom=0.8
            ),
            (125, 1, 1, 255),
        ),
        (
            FlashCut(start=0.5, duration=1, colour="#ffffff", intensity=0.5),
            (128, 128, 255, 255),
        ),
        (
            DirectionalPush(
                start=0.5, duration=1, angle_degrees=90, distance=1, blur_radius=2
            ),
            (125, 1, 1, 255),
        ),
        (
            ZoomBlur(
                start=0.5,
                duration=1,
                outgoing_zoom=1.2,
                incoming_start_zoom=0.8,
                blur_radius=2,
            ),
            (114, 1, 22, 255),
        ),
    ],
)
def test_transition_variants_render_through_high_level_api(
    transition: object, expected: tuple[int, int, int, int]
) -> None:
    project = vestra.Project(size=(8, 6), fps=10, duration=2, base_directory=".")
    outgoing = project.root.add(Image("examples/assets/red.png", sizing=Sizing.cover()))
    incoming = project.root.add(
        Image("examples/assets/blue.png", sizing=Sizing.cover())
    )
    project.root.transitions.add(outgoing, incoming, transition)  # type: ignore[arg-type]
    frame = project.render_frame(1.0, backend="cpu").to_bytes()
    assert tuple(frame[:4]) == expected
