"""Typed source-local procedural primitive shapes."""

from __future__ import annotations

from math import isfinite
from typing import TypeAlias

from ..authoring.values import Color, Point, color_to_canonical
from .base import Source

PointValue: TypeAlias = Point | tuple[int | float, int | float] | list[int | float]


def _number(value: int | float, name: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    result = float(value)
    if not isfinite(result) or (positive and result <= 0.0):
        raise ValueError(f"{name} must be finite and {'positive' if positive else 'valid'}")
    return result


def _point(value: PointValue, name: str) -> Point:
    if isinstance(value, Point):
        return value
    if not isinstance(value, (tuple, list)) or len(value) != 2:
        raise TypeError(f"{name} must be a Point or a two-item coordinate")
    return Point(value[0], value[1])


class Shape(Source):
    """Base class for static source-local primitive shapes."""

    __slots__ = ("_fill", "_stroke", "_stroke_width")

    def __init__(
        self,
        *,
        fill: Color | str | None,
        stroke: Color | str | None,
        stroke_width: int | float,
    ) -> None:
        if fill is None and stroke is None:
            raise ValueError("shape must have a fill or stroke")
        self.fill = fill
        self.stroke = stroke
        self.stroke_width = stroke_width

    @property
    def fill(self) -> str | None:
        return self._fill

    @fill.setter
    def fill(self, value: Color | str | None) -> None:
        self._fill = None if value is None else color_to_canonical(value)

    @property
    def stroke(self) -> str | None:
        return self._stroke

    @stroke.setter
    def stroke(self, value: Color | str | None) -> None:
        self._stroke = None if value is None else color_to_canonical(value)

    @property
    def stroke_width(self) -> float:
        return self._stroke_width

    @stroke_width.setter
    def stroke_width(self, value: int | float) -> None:
        width = _number(value, "stroke_width")
        if self.stroke is not None and width <= 0.0:
            raise ValueError("stroke_width must be positive when stroke is enabled")
        if width < 0.0:
            raise ValueError("stroke_width must not be negative")
        self._stroke_width = width

    def _canonical_style(self) -> dict[str, object]:
        data: dict[str, object] = {}
        if self.fill is not None:
            data["fill"] = self.fill
        if self.stroke is not None:
            data["stroke"] = self.stroke
        if self.stroke_width:
            data["stroke_width"] = self.stroke_width
        return data

    def to_canonical(self) -> dict[str, object]:
        raise NotImplementedError


class Rectangle(Shape):
    """A source-local axis-aligned rectangle."""

    __slots__ = ("_width", "_height", "_corner_radius")

    def __init__(self, *, width: int | float, height: int | float, fill: Color | str | None = None,
                 stroke: Color | str | None = None, stroke_width: int | float = 0.0,
                 corner_radius: int | float = 0.0) -> None:
        self.width = width
        self.height = height
        self.corner_radius = corner_radius
        super().__init__(fill=fill, stroke=stroke, stroke_width=stroke_width)

    @property
    def width(self) -> float: return self._width
    @width.setter
    def width(self, value: int | float) -> None: self._width = _number(value, "width", positive=True)
    @property
    def height(self) -> float: return self._height
    @height.setter
    def height(self, value: int | float) -> None: self._height = _number(value, "height", positive=True)
    @property
    def corner_radius(self) -> float: return self._corner_radius
    @corner_radius.setter
    def corner_radius(self, value: int | float) -> None:
        radius = _number(value, "corner_radius")
        if radius < 0.0 or radius > min(self.width, self.height) / 2.0:
            raise ValueError("corner_radius must be between zero and half the shorter side")
        self._corner_radius = radius

    def to_canonical(self) -> dict[str, object]:
        geometry: dict[str, object] = {"type": "rectangle", "width": self.width, "height": self.height}
        if self.corner_radius:
            geometry["corner_radius"] = self.corner_radius
        return {"type": "shape", "geometry": geometry, **self._canonical_style()}


class Ellipse(Shape):
    """A source-local ellipse."""

    __slots__ = ("_width", "_height")

    def __init__(self, *, width: int | float, height: int | float, fill: Color | str | None = None,
                 stroke: Color | str | None = None, stroke_width: int | float = 0.0) -> None:
        self.width = _number(width, "width", positive=True)
        self.height = _number(height, "height", positive=True)
        super().__init__(fill=fill, stroke=stroke, stroke_width=stroke_width)

    @property
    def width(self) -> float: return self._width
    @width.setter
    def width(self, value: float) -> None: self._width = value
    @property
    def height(self) -> float: return self._height
    @height.setter
    def height(self, value: float) -> None: self._height = value

    def to_canonical(self) -> dict[str, object]:
        return {"type": "shape", "geometry": {"type": "ellipse", "width": self.width, "height": self.height}, **self._canonical_style()}


class Circle(Ellipse):
    """Ellipse authoring convenience with equal dimensions."""

    def __init__(self, *, radius: int | float, fill: Color | str | None = None,
                 stroke: Color | str | None = None, stroke_width: int | float = 0.0) -> None:
        diameter = _number(radius, "radius", positive=True) * 2.0
        super().__init__(width=diameter, height=diameter, fill=fill, stroke=stroke, stroke_width=stroke_width)


class Line(Shape):
    """An open stroked line in source-local coordinates."""

    __slots__ = ("_start", "_end")

    def __init__(self, *, start: PointValue, end: PointValue, stroke: Color | str,
                 stroke_width: int | float) -> None:
        self._start = _point(start, "start")
        self._end = _point(end, "end")
        if self._start == self._end:
            raise ValueError("line endpoints must be distinct")
        super().__init__(fill=None, stroke=stroke, stroke_width=stroke_width)

    @property
    def start(self) -> Point:
        return self._start

    @property
    def end(self) -> Point:
        return self._end

    def to_canonical(self) -> dict[str, object]:
        return {"type": "shape", "geometry": {"type": "line", "start": self._start.to_canonical(), "end": self._end.to_canonical()}, **self._canonical_style()}


class Polygon(Shape):
    """A closed polygon with a source-local vertex list."""

    __slots__ = ("_points",)

    def __init__(self, *, points: list[PointValue] | tuple[PointValue, ...], fill: Color | str | None = None,
                 stroke: Color | str | None = None, stroke_width: int | float = 0.0) -> None:
        if len(points) < 3:
            raise ValueError("polygon needs at least three points")
        self._points = tuple(_point(point, "points item") for point in points)
        super().__init__(fill=fill, stroke=stroke, stroke_width=stroke_width)

    @property
    def points(self) -> tuple[Point, ...]: return self._points

    def to_canonical(self) -> dict[str, object]:
        return {"type": "shape", "geometry": {"type": "polygon", "points": [point.to_canonical() for point in self.points]}, **self._canonical_style()}


__all__ = ["Shape", "Rectangle", "Ellipse", "Circle", "Line", "Polygon"]
