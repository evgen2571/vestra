"""Immutable, explicitly serializable values used by the authoring API."""

from dataclasses import dataclass
from enum import Enum
from math import isfinite
import re
from typing import Final, Literal


_COLOUR: Final[re.Pattern[str]] = re.compile(r"^#[0-9A-Fa-f]{6}(?:[0-9A-Fa-f]{2})?$")


def _finite_number(value: int | float, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{name} must be a real number")
    number = float(value)
    if not isfinite(number):
        raise ValueError(f"{name} must be finite")
    return number


class _CanonicalStrEnum(Enum):
    def to_canonical(self) -> str:
        return str(self.value)


class Interpolation(_CanonicalStrEnum):
    LINEAR = "linear"
    HOLD = "hold"
    EASE_IN = "ease_in"
    EASE_OUT = "ease_out"
    EASE_IN_OUT = "ease_in_out"


@dataclass(frozen=True, slots=True)
class Sizing:
    """Immutable image sizing with the schema's tagged representation."""

    mode: Literal["original", "fit", "cover", "scale", "stretch"]
    scale_value: float | None = None
    width: int | None = None
    height: int | None = None

    def __post_init__(self) -> None:
        if self.mode not in {"original", "fit", "cover", "scale", "stretch"}:
            raise ValueError("sizing mode is invalid")
        if self.mode in {"original", "fit", "cover"}:
            if self.scale_value is not None or self.width is not None or self.height is not None:
                raise ValueError(f"{self.mode} sizing does not accept parameters")
            return
        if self.mode == "scale":
            if self.scale_value is None or self.width is not None or self.height is not None:
                raise ValueError("scale sizing requires only scale")
            scale = _finite_number(self.scale_value, "scale")
            if scale <= 0:
                raise ValueError("scale must be positive")
            object.__setattr__(self, "scale_value", scale)
            return
        if self.width is None or self.height is None or self.scale_value is not None:
            raise ValueError("stretch sizing requires only width and height")
        for name in ("width", "height"):
            value = getattr(self, name)
            if isinstance(value, bool) or not isinstance(value, int):
                raise TypeError(f"{name} must be an integer")
            if value < 1:
                raise ValueError(f"{name} must be positive")

    @classmethod
    def original(cls) -> "Sizing":
        return cls("original")

    @classmethod
    def fit(cls) -> "Sizing":
        return cls("fit")

    @classmethod
    def cover(cls) -> "Sizing":
        return cls("cover")

    @classmethod
    def scale(cls, value: int | float) -> "Sizing":
        return cls("scale", scale_value=value)

    @classmethod
    def stretch(cls, *, width: int, height: int) -> "Sizing":
        return cls("stretch", width=width, height=height)

    def to_canonical(self) -> dict[str, str | float | int]:
        if self.mode == "scale":
            assert self.scale_value is not None
            return {"mode": self.mode, "scale": self.scale_value}
        if self.mode == "stretch":
            assert self.width is not None and self.height is not None
            return {"mode": self.mode, "width": self.width, "height": self.height}
        return {"mode": self.mode}


class BlendMode(_CanonicalStrEnum):
    NORMAL = "normal"
    ADD = "add"
    SCREEN = "screen"
    MULTIPLY = "multiply"
    OVERLAY = "overlay"


class Quality(_CanonicalStrEnum):
    PREVIEW = "preview"
    BALANCED = "balanced"
    HIGH = "high"


class DurationMode(_CanonicalStrEnum):
    AUTOMATIC = "automatic"
    EXPLICIT = "explicit"


@dataclass(frozen=True, slots=True)
class Color:
    """A canonical ``#RRGGBB`` or ``#RRGGBBAA`` colour."""

    value: str

    def __post_init__(self) -> None:
        if not isinstance(self.value, str):
            raise TypeError("color must be a string")
        if _COLOUR.fullmatch(self.value) is None:
            raise ValueError("color must use #RRGGBB or #RRGGBBAA")
        object.__setattr__(self, "value", self.value.lower())

    def to_canonical(self) -> str:
        return self.value


def color_to_canonical(value: Color | str) -> str:
    if isinstance(value, Color):
        return value.to_canonical()
    return Color(value).to_canonical()


@dataclass(frozen=True, slots=True)
class Point:
    x: float
    y: float

    def __post_init__(self) -> None:
        object.__setattr__(self, "x", _finite_number(self.x, "x"))
        object.__setattr__(self, "y", _finite_number(self.y, "y"))

    def to_canonical(self) -> dict[str, float]:
        return {"x": self.x, "y": self.y}


@dataclass(frozen=True, slots=True)
class Crop:
    x: float
    y: float
    width: float
    height: float

    def __post_init__(self) -> None:
        for name in ("x", "y", "width", "height"):
            object.__setattr__(self, name, _finite_number(getattr(self, name), name))

    def to_canonical(self) -> dict[str, float]:
        return {"x": self.x, "y": self.y, "width": self.width, "height": self.height}


@dataclass(frozen=True, slots=True)
class CubicBezier:
    x1: float
    y1: float
    x2: float
    y2: float

    def __post_init__(self) -> None:
        for name in ("x1", "y1", "x2", "y2"):
            object.__setattr__(self, name, _finite_number(getattr(self, name), name))
        if not 0.0 <= self.x1 <= 1.0 or not 0.0 <= self.x2 <= 1.0:
            raise ValueError("cubic Bézier x controls must be between 0 and 1")

    def to_canonical(self) -> dict[str, float | str]:
        return {
            "type": "cubic_bezier",
            "x1": self.x1,
            "y1": self.y1,
            "x2": self.x2,
            "y2": self.y2,
        }
