"""Public analog effects retain independent state and canonical contracts."""
import pytest
import vestra
from vestra import effects
from vestra.authoring import ProjectBuilder
from vestra.effects.recipes import analog_monitor, halftone_print, sorted_neon


@pytest.mark.parametrize("effect,kind", [(effects.Halftone(), "halftone"), (effects.PixelSort(), "pixel_sort"), (effects.Crt(period=2), "crt")])
def test_analog_effects_roundtrip_and_animate_without_shared_state(effect, kind):
    project = vestra.Project(size=(8, 8), fps=2, duration=1)
    layer = project.root.add(vestra.sources.Color("#80808080"), duration=1)
    attached = layer.effects.add(effect)
    attached.amount.keyframe(1, 0.5)
    assert not effect.amount.keyframes
    data = project.snapshot().to_dict()
    assert data["visual"]["clips"][0]["effects"][0]["type"] == kind
    assert vestra.ProjectSnapshot.from_json(project.snapshot().to_json()).to_dict() == data


def test_analog_enums_and_invalid_bounds_are_checked():
    assert effects.Halftone(mode="rgb").mode is effects.HalftoneMode.RGB
    sort = effects.PixelSort(direction="vertical", order="descending")
    assert sort.direction is effects.PixelSortDirection.VERTICAL
    assert sort.order is effects.PixelSortOrder.DESCENDING
    for cls, values in [(effects.Halftone, {"cell_size": 1}), (effects.Halftone, {"softness": 3}), (effects.PixelSort, {"segment_length": 257}), (effects.PixelSort, {"direction": "diagonal"}), (effects.Crt, {"period": 0}), (effects.Crt, {"mask_spacing": 0})]:
        with pytest.raises((TypeError, ValueError)):
            cls(**values)


def test_advanced_analog_factories_expose_tracks_and_native_snapshot():
    builder = ProjectBuilder(width=8, height=8, frame_rate=vestra.FrameRate(2, 1), output_path="out.mp4", duration=1)
    builder.add_solid_color_clip(colour="#808080", start=0, duration=1, layer=0)
    halftone = builder.post_effects.add_halftone(mode="source")
    sort = builder.post_effects.add_pixel_sort(direction="vertical")
    crt = builder.post_effects.add_crt(period=2)
    halftone.cell_size.keyframe(time=1, value=8)
    sort.amount.keyframe(time=1, value=0.5)
    crt.phase.keyframe(time=1, value=0.25)
    assert halftone.to_canonical()["cell_size"]["keyframes"]
    assert sort.to_canonical()["direction"] == "vertical"
    assert crt.to_canonical()["period"] == 2
    builder.build()


@pytest.mark.parametrize("recipe", [halftone_print, analog_monitor, sorted_neon])
def test_recipes_create_fresh_ordinary_effects_and_lower_on_global_stack(recipe):
    first = recipe()
    second = recipe()
    assert all(a is not b for a, b in zip(first, second))
    project = vestra.Project(size=(8, 8), fps=2, duration=1)
    project.root.add(vestra.sources.Color("#808080"), duration=1)
    for effect in first:
        project.post_effects.add(effect)
    assert len(project.snapshot().to_dict()["visual"]["post_effects"]) == len(first)
