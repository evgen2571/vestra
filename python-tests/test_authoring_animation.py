"""Phase 8C-A keyframe authoring, serialization, and native validation."""

from copy import deepcopy

import pytest

from video_editor import FrameRate
from video_editor.authoring import (
    Crop, CropKeyframe, CubicBezier, Interpolation, Point, PointKeyframe,
    ProjectBuilder, ScalarKeyframe,
)


def builder() -> ProjectBuilder:
    return ProjectBuilder(
        width=160, height=90, frame_rate=FrameRate(10, 1), output_path="out.mp4", duration=2,
    )


def image_clip() -> tuple[ProjectBuilder, object]:
    authored = builder()
    asset = authored.add_image_asset("examples/assets/red.png")
    return authored, authored.add_image_clip(source=asset, start=0, duration=2, layer=0)


def test_keyframes_are_immutable_typed_snapshots_and_serialize_canonically() -> None:
    authored, clip = image_clip()
    scalar: ScalarKeyframe = clip.opacity.keyframe(time=0, value=0, interpolation=Interpolation.EASE_OUT)
    point: PointKeyframe = clip.transform.scale.keyframe(
        time=1, value=Point(1.2, 1.2), interpolation=CubicBezier(0.25, 0.1, 0.25, 1),
    )
    crop: CropKeyframe = clip.crop.keyframe(time=0.5, value=Crop(0, 0, 0.8, 1))
    assert scalar == ScalarKeyframe(0, 0, Interpolation.EASE_OUT)
    assert "object at" not in repr(point)
    with pytest.raises(AttributeError):
        scalar.time = 1  # type: ignore[misc]
    data = authored.to_dict()
    tracks = data["visual"]["clips"][0]
    assert tracks["opacity"]["keyframes"] == [{"time": 0.0, "value": 0.0, "interpolation": "ease_out"}]
    assert tracks["transform"]["scale"]["keyframes"] == [{
        "time": 1.0, "value": {"x": 1.2, "y": 1.2},
        "interpolation": {"type": "cubic_bezier", "x1": 0.25, "y1": 0.1, "x2": 0.25, "y2": 1.0},
    }]
    assert "crop" not in tracks
    clip.set_crop(Crop(0, 0, 1, 1))
    assert clip.crop.keyframes == (crop,)


def test_order_is_preserved_and_failed_mutation_is_transactional() -> None:
    authored, clip = image_clip()
    first = clip.opacity.keyframe(time=1, value=0.2)
    second = clip.opacity.keyframe(time=0.5, value=0.5)
    assert clip.opacity.keyframes == (first, second)
    before = deepcopy(authored.to_dict())
    with pytest.raises(ValueError):
        clip.opacity.keyframe(time=float("nan"), value=1)
    with pytest.raises(ValueError):
        clip.transform.scale.keyframe(time=1, value=Point(0, 1))
    with pytest.raises(TypeError):
        clip.opacity.keyframe(time=1, value=Point(1, 1))  # type: ignore[arg-type]
    assert authored.to_dict() == before
    held = clip.opacity.keyframes
    clip.opacity.clear_keyframes()
    assert clip.opacity.keyframes == () and held == (first, second)


def test_specialized_validation_and_crop_lifecycle_are_preserved() -> None:
    authored, clip = image_clip()
    with pytest.raises(ValueError, match="opacity"):
        clip.opacity.keyframe(time=0, value=2)
    with pytest.raises(ValueError, match="anchor"):
        clip.transform.anchor.keyframe(time=0, value=Point(-0.1, 0.5))
    with pytest.raises(ValueError, match="scale"):
        clip.transform.scale.keyframe(time=0, value=Point(-1, 1))
    with pytest.raises(ValueError, match="Bézier"):
        CubicBezier(-0.1, 0, 1, 1)
    keyframe = clip.crop.keyframe(time=1, value=Crop(0, 0, 1, 1))
    clip.clear_crop()
    assert not clip.has_crop and clip.crop.keyframes == (keyframe,)
    clip.set_crop(Crop(0, 0, 1, 1))
    assert clip.has_crop and clip.crop.keyframes == (keyframe,)
    assert authored.validate().is_valid


@pytest.mark.parametrize("times", [(1.0, 0.5), (0.5, 0.5), (2.1,)])
def test_native_validation_keeps_invalid_authored_timing_visible(times: tuple[float, ...]) -> None:
    authored, clip = image_clip()
    for time in times:
        clip.opacity.keyframe(time=time, value=0.5)
    report = authored.validate()
    assert not report.is_valid
    assert any(diagnostic.code == "MVP-KEYFRAME-TIME" for diagnostic in report.errors)
