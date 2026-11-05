"""Sized groups retain their local dimensions through every authoring path."""

from typing import Any, cast

import pytest

import vestra
from vestra.authoring import ProjectBuilder
from vestra.sources import Group, Rectangle


def test_group_size_serializes_nested_sources_and_owned_masks() -> None:
    inner = Group([Rectangle(width=20, height=10, fill="#2563eb")], size=(100.5, 50))
    outer = Group([inner], size=(600, 300))
    data = outer.to_canonical(inherited_duration=2)
    assert outer.size == (600.0, 300.0)
    assert data["size"] == {"width": 600.0, "height": 300.0}
    assert data["clips"][0]["source"]["size"] == {"width": 100.5, "height": 50.0}  # type: ignore[index]
    assert "size" not in Group().to_canonical()
    mask = Group([Rectangle(width=30, height=15, fill="#ffffff")], size=(60, 30))
    child = outer.add(Rectangle(width=30, height=15, fill="#ff0000"))
    child.masks.add(mask)
    value = outer.to_canonical(inherited_duration=2)
    assert value["clips"][1]["masks"][0]["input"]["source"]["size"] == {"width": 60.0, "height": 30.0}  # type: ignore[index]


@pytest.mark.parametrize("size", [(0, 20), (-1, 20), (2**32, 20), (2**32 - 1, 2**32 - 1), (20, float("nan")), (float("inf"), 20), (20, float("-inf")), (10**1000, 20)])
def test_group_rejects_nonpositive_or_nonfinite_size(size: Any) -> None:
    with pytest.raises(ValueError, match="Group size"):
        Group(size=size)


@pytest.mark.parametrize("size", [(True, 20), (20, False), ("20", 20), (20,), (20, 10, 5), [20, 10], "20,10"])
def test_group_rejects_invalid_size_types(size: Any) -> None:
    with pytest.raises(TypeError, match="Group size"):
        Group(size=size)


def test_group_size_is_read_only() -> None:
    source = Group(size=(60, 30))
    with pytest.raises(AttributeError):
        source.size = (120, 60)  # type: ignore[misc]


def test_group_can_author_large_representable_dimensions_before_resource_validation() -> None:
    source = Group(size=(16_384, 2))
    assert source.to_canonical()["size"] == {"width": 16_384.0, "height": 2.0}


def test_group_resource_limits_are_safe_by_default_and_apply_to_native_and_high_level(tmp_path: Any) -> None:
    source = Group(size=(16_384, 2))
    source.add(Rectangle(width=16_384, height=2, fill="#ff0000"))
    default = vestra.Project(size=(2, 2), fps=2, duration=0.5)
    default.root.add(source, scale=(2 / 16_384, 1))
    assert not default.validate().is_valid
    assert any(d.code == "VESTRA-LIMIT-GROUP-DIMENSIONS" for d in default.validate().diagnostics)
    limits = vestra.ResourceLimits(maximum_group_width=16_384, maximum_group_pixels=32_768)
    assert limits.maximum_group_width == 16_384
    assert limits.maximum_group_height == 8192
    assert limits.maximum_group_pixels == 32_768
    editor = vestra.Editor(resource_limits=limits)
    snapshot = default.snapshot()
    assert editor.validate(snapshot).is_valid
    assert editor.preflight(snapshot, vestra.PreflightOptions.for_inspection()).is_valid
    frame = editor.prepare(snapshot, vestra.PrepareOptions(backend=vestra.BackendPreference.CPU)).render_frame_number(0)
    assert frame.to_bytes() == bytes((255, 0, 0, 255)) * 4
    configured = vestra.Project(size=(2, 2), fps=2, duration=0.5, resource_limits=limits)
    configured.root.add(source, scale=(2 / 16_384, 1))
    assert configured.validate().is_valid
    assert configured.snapshot().to_dict() == snapshot.to_dict()
    assert configured.render_frame(0, backend="cpu").to_bytes() == frame.to_bytes()
    result = configured.render(tmp_path / "large-thin.mp4", backend="cpu", show_progress=False)
    assert result.output_path.is_file()
    limited = vestra.Editor(resource_limits=vestra.ResourceLimits(maximum_group_width=16_384, maximum_group_pixels=32_767))
    assert any(d.code == "VESTRA-LIMIT-GROUP-PIXELS" for d in limited.validate(snapshot).diagnostics)
    with pytest.raises(vestra.PreparationError, match="group canvas pixels"):
        limited.prepare(snapshot, vestra.PrepareOptions(backend=vestra.BackendPreference.CPU))


@pytest.mark.parametrize("value", [True, 1.5, "8192"])
def test_group_resource_limits_reject_noninteger_values(value: Any) -> None:
    for name in ("maximum_group_width", "maximum_group_height", "maximum_group_pixels"):
        with pytest.raises(TypeError):
            vestra.ResourceLimits(**{name: value})


def test_group_resource_limits_reject_out_of_range_and_are_frozen() -> None:
    with pytest.raises(ValueError):
        vestra.ResourceLimits(maximum_group_width=2**32)
    with pytest.raises(ValueError):
        vestra.ResourceLimits(maximum_group_pixels=2**64)
    with pytest.raises(ValueError):
        vestra.ResourceLimits(maximum_group_height=-1)
    with pytest.raises(AttributeError):
        vestra.ResourceLimits().maximum_group_width = 16_384  # type: ignore[misc]


def test_advanced_group_size_validation_is_atomic_and_serializes() -> None:
    builder = ProjectBuilder(width=32, height=24, frame_rate=vestra.FrameRate(24, 1), output_path="out.mp4", duration=2)
    child = builder.add_solid_color_clip(colour="#ffffff", start=0, duration=2, layer=0)
    original = builder.to_dict()
    with pytest.raises(ValueError, match="Group size"):
        builder.add_group_clip(clips=[child], size=(0, 10), start=0, duration=2, layer=0)
    assert builder.to_dict() == original
    group = builder.add_group_clip(clips=[child], size=(100.5, 50), start=0, duration=2, layer=0)
    assert group.size == (100.5, 50.0)
    assert group.to_canonical()["source"]["size"] == {"width": 100.5, "height": 50.0}  # type: ignore[index]
    with pytest.raises(AttributeError):
        group.size = (120, 60)  # type: ignore[misc]


def test_group_size_survives_lowering_reuse_and_snapshot_round_trip() -> None:
    source = Group([Rectangle(width=20, height=10, fill="#2563eb")], size=(60, 30))
    first = vestra.Project(size=(160, 90), fps=24, duration=2)
    layer = first.root.add(source, position=(0.25, 0.5))
    second = vestra.Project(size=(320, 180), fps=30, duration=3)
    second_layer = second.root.add(source, position=(0.75, 0.5))
    second_layer.masks.add(source)
    source.add(Rectangle(width=5, height=5, fill="#ff0000"))
    assert cast(Group, layer.source).size == (60.0, 30.0)
    assert len(cast(Group, layer.source).children) == 1
    assert len(cast(Group, second_layer.source).children) == 1
    for project in (first, second):
        snapshot = project.snapshot()
        data = snapshot.to_dict()
        assert data["visual"]["clips"][0]["source"]["size"] == {"width": 60.0, "height": 30.0}
        assert vestra.ProjectSnapshot.from_json(snapshot.to_json()).to_dict() == data
        assert project.validate().is_valid
    masks = second.snapshot().to_dict()["visual"]["clips"][0]["masks"]
    assert masks[0]["input"]["source"]["size"] == {"width": 60.0, "height": 30.0}
