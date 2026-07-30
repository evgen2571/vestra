from dataclasses import FrozenInstanceError

import pytest

from video_editor.authoring import (
    BlendMode,
    Color,
    Crop,
    CubicBezier,
    DurationMode,
    Interpolation,
    Point,
    Quality,
    Sizing,
)


def test_values_serialize_with_the_canonical_shapes() -> None:
    assert Color("#AAbbCc").to_canonical() == "#aabbcc"
    assert Color("#AABBCCDD").to_canonical() == "#aabbccdd"
    assert Point(1, 2).to_canonical() == {"x": 1.0, "y": 2.0}
    assert Crop(0, 0.25, 1, 0.75).to_canonical() == {
        "x": 0.0, "y": 0.25, "width": 1.0, "height": 0.75,
    }
    assert CubicBezier(0.1, 0, 0.9, 1).to_canonical() == {
        "type": "cubic_bezier", "x1": 0.1, "y1": 0.0, "x2": 0.9, "y2": 1.0,
    }
    assert Interpolation.EASE_IN.to_canonical() == "ease_in"
    assert Sizing.original().to_canonical() == {"mode": "original"}
    assert Sizing.fit().to_canonical() == {"mode": "fit"}
    assert Sizing.cover().to_canonical() == {"mode": "cover"}
    assert Sizing.scale(1.25).to_canonical() == {"mode": "scale", "scale": 1.25}
    assert Sizing.stretch(width=1280, height=720).to_canonical() == {
        "mode": "stretch", "width": 1280, "height": 720,
    }
    assert BlendMode.OVERLAY.to_canonical() == "overlay"
    assert Quality.HIGH.to_canonical() == "high"
    assert DurationMode.EXPLICIT.to_canonical() == "explicit"


@pytest.mark.parametrize("value", ["red", "#123", "#gg0000"])
def test_color_rejects_noncanonical_strings(value: str) -> None:
    with pytest.raises(ValueError):
        Color(value)


@pytest.mark.parametrize("value", [True, float("nan"), float("inf")])
def test_numeric_values_reject_boolean_and_nonfinite_values(value: object) -> None:
    with pytest.raises((TypeError, ValueError)):
        Point(value, 0)  # type: ignore[arg-type]


def test_values_are_frozen() -> None:
    point = Point(1, 2)
    with pytest.raises((FrozenInstanceError, AttributeError)):
        point.x = 3  # type: ignore[misc]


@pytest.mark.parametrize(
    ("factory", "error"),
    [
        (lambda: Sizing.scale(True), TypeError),
        (lambda: Sizing.scale(float("nan")), ValueError),
        (lambda: Sizing.scale(float("inf")), ValueError),
        (lambda: Sizing.scale(0), ValueError),
        (lambda: Sizing.stretch(width=True, height=720), TypeError),
        (lambda: Sizing.stretch(width=1280, height="720"), TypeError),  # type: ignore[arg-type]
        (lambda: Sizing.stretch(width=0, height=720), ValueError),
    ],
)
def test_sizing_rejects_invalid_parameters(factory: object, error: type[Exception]) -> None:
    with pytest.raises(error):
        factory()  # type: ignore[operator]


def test_sizing_is_frozen_and_has_value_equality() -> None:
    sizing = Sizing.scale(1.25)
    assert sizing == Sizing.scale(1.25)
    with pytest.raises((FrozenInstanceError, AttributeError)):
        sizing.mode = "cover"  # type: ignore[misc]
