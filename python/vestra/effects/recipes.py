"""Optional ordered effect chains for footage, groups and global output.

Each call creates fresh, mutable effects. Attach with ``for effect in
analog_monitor(): clip.effects.add(effect)`` and edit the returned handles.
"""
from .analog import Crt, Halftone, PixelSort
from .base import Effect
from .palette import OrderedDither
from .stylize import Bloom, ColorAdjust


def halftone_print() -> tuple[Effect, ...]:
    """A source-colored, antialiased printed-screen look."""
    return (ColorAdjust(exposure=0.1, gamma=0.9, black_point=0, white_point=1), Halftone(mode="source", cell_size=6))


def analog_monitor(*, period: float | None = None) -> tuple[Effect, ...]:
    """Fine phosphor-style dithering, restrained bloom and continuous CRT motion."""
    return (
        OrderedDither(("#04120b", "#255941", "#79b57e", "#e4f4ce")),
        Bloom(threshold=0.65, radius=3, intensity=0.15),
        Crt(curvature=0.04, grain=0.01, jitter=0.15, period=period),
    )


def sorted_neon(*, direction: str = "horizontal") -> tuple[Effect, ...]:
    """Sharp bounded sorting followed by fine palette texture and a soft glow."""
    return (
        PixelSort(direction=direction, lower_threshold=0.2, upper_threshold=0.85, amount=0.7),
        OrderedDither(("#0d0920", "#67294d", "#cc638f", "#fff0ba")),
        Bloom(threshold=0.7, radius=4, intensity=0.25),
    )
