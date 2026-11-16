"""Finite-number refactors preserve public input policies and messages."""

import math

import pytest
import vestra
from vestra.authoring import Point
from vestra.flashes import Flash
from vestra.presets import Preset
from vestra.transitions import DirectionalPush, ZoomCrossfade


FACTORIES = [
    (lambda value: vestra.ScalarProperty(value).value, "value", "finite"),
    (lambda value: vestra.Brightness(value).amount.value, "amount", "finite"),
    (lambda value: Point(value, 0).x, "x", "finite"),
    (lambda value: DirectionalPush(angle_degrees=value).angle_degrees, "angle_degrees", "finite"),
    (lambda value: ZoomCrossfade(outgoing_zoom=value).outgoing_zoom, "outgoing_zoom", "finite"),
    (lambda value: Flash(0, 1, "#ffffff", opacity=value).opacity, "opacity", "finite"),
    (lambda value: Preset("slow_drift", intensity=value).intensity, "intensity", "finite"),
    (lambda value: vestra.Project(size=(2, 2), fps=1, duration=value).duration, "duration", "finite"),
    (lambda value: vestra.Project(size=(2, 2), fps=1, duration=1).audio.track(gain=value).gain, "gain", "finite"),
    (lambda value: vestra.Text("text", font_size=value).font_size, "font_size", "finite and positive"),
    (lambda value: vestra.Rectangle(width=value).width, "width", "finite and positive"),
    (lambda value: vestra.Rectangle(stroke_width=value).stroke_width, "stroke_width", "finite and valid"),
]


@pytest.mark.parametrize("factory,name,finite", FACTORIES)
@pytest.mark.parametrize("value", [True, False, "1", None, 1j, object()])
def test_real_input_types_keep_their_policy(factory, name, finite, value):
    if name == "duration" and value is None:
        assert factory(value) is None
        return
    with pytest.raises(TypeError) as error:
        factory(value)
    assert str(error.value) == f"{name} must be a real number"


@pytest.mark.parametrize("factory,name,finite", FACTORIES)
@pytest.mark.parametrize("value", [math.nan, math.inf, -math.inf])
def test_nonfinite_reals_keep_their_exception_and_message(factory, name, finite, value):
    with pytest.raises(ValueError) as error:
        factory(value)
    assert str(error.value) == f"{name} must be {finite}"


@pytest.mark.parametrize("factory,name,finite", FACTORIES)
@pytest.mark.parametrize("value", [1, 0.5])
def test_finite_integer_and_float_inputs_are_preserved(factory, name, finite, value):
    assert factory(value) == float(value)


@pytest.mark.parametrize("factory,message", [
    (lambda value: ZoomCrossfade(outgoing_zoom=value), "outgoing_zoom must be positive"),
    (lambda value: vestra.Project(size=(2, 2), fps=1, duration=value), "duration must be positive"),
    (lambda value: vestra.Text("text", font_size=value), "font_size must be finite and positive"),
    (lambda value: vestra.Rectangle(width=value), "width must be finite and positive"),
])
@pytest.mark.parametrize("value", [0, -0.5])
def test_positive_boundaries_remain_strict(factory, message, value):
    with pytest.raises(ValueError) as error:
        factory(value)
    assert str(error.value) == message


def test_nonnegative_and_caller_specific_ranges_remain_distinct():
    assert DirectionalPush(distance=0).distance == 0
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    assert project.audio.track(gain=0).gain == 0
    assert Preset("slow_drift", intensity=0).intensity == 0
    assert Flash(0, 1, "#ffffff", opacity=0).opacity == 0
    for factory, message in [
        (lambda: DirectionalPush(distance=-1), "distance must be non-negative"),
        (lambda: project.audio.track(gain=-1), "gain must be non-negative"),
        (lambda: Preset("slow_drift", intensity=3), "intensity must be between 0 and 2"),
        (lambda: Flash(0, 1, "#ffffff", opacity=2), "opacity must be between 0 and 1"),
    ]:
        with pytest.raises(ValueError) as error:
            factory()
        assert str(error.value) == message
