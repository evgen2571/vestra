"""Small deterministic ParticleSystem presets for common authoring tasks."""

from .particles import (
    CircleEmitter, ColourLifetimeStop, ParticleBlendMode, ParticleBurst,
    ParticleLifetimeStyle, ParticleSystem, PointEmitter,
    RectangleEmitter, ScalarLifetimeStop, ScalarRange,
)
from .values import Point


def ambient_stars(*, seed: int = 17, density: float = 18.0) -> ParticleSystem:
    """Return a quiet field of small, mostly stationary stars."""
    return ParticleSystem(
        seed=seed, emitter=RectangleEmitter(Point(0.5, 0.5), Point(1.0, 1.0)), rate=density,
        lifetime=8.0, size=0.008, size_range=ScalarRange(0.004, 0.012),
        opacity=0.65, blend_mode=ParticleBlendMode.ADDITIVE,
        lifetime_style=ParticleLifetimeStyle(opacity=(ScalarLifetimeStop(0.0, 0.0), ScalarLifetimeStop(0.15, 1.0), ScalarLifetimeStop(1.0, 0.0))),
    )


def snow(*, seed: int = 23, density: float = 24.0) -> ParticleSystem:
    """Return constant-velocity snow falling from just above the canvas."""
    return ParticleSystem(
        seed=seed, emitter=RectangleEmitter(Point(0.5, -0.05), Point(1.1, 0.1)), rate=density,
        lifetime=5.0, size=0.012, size_range=ScalarRange(0.006, 0.018),
        speed=0.16, speed_range=ScalarRange(0.10, 0.22), direction=90.0, spread=8.0,
        colour="#e8f3ff", blend_mode=ParticleBlendMode.NORMAL,
    )


def embers(*, seed: int = 31, density: float = 12.0) -> ParticleSystem:
    """Return warm embers rising from the lower centre."""
    return ParticleSystem(
        seed=seed, emitter=RectangleEmitter(Point(0.5, 1.0), Point(0.25, 0.06)), rate=density,
        lifetime=2.5, size=0.01, size_range=ScalarRange(0.004, 0.016),
        speed=0.16, speed_range=ScalarRange(0.10, 0.24), direction=270.0, spread=24.0,
        colour="#ff6a24", blend_mode=ParticleBlendMode.ADDITIVE,
        lifetime_style=ParticleLifetimeStyle(
            opacity=(ScalarLifetimeStop(0.0, 0.0), ScalarLifetimeStop(0.12, 1.0), ScalarLifetimeStop(1.0, 0.0)),
            colour=(ColourLifetimeStop(0.0, "#ff3b12"), ColourLifetimeStop(0.65, "#ffcf33"), ColourLifetimeStop(1.0, "#3b1510")),
        ),
    )


def sparks(*, seed: int = 41, count: int = 32) -> ParticleSystem:
    """Return a short deterministic full-circle spark burst."""
    return ParticleSystem(
        seed=seed, emitter=PointEmitter(Point(0.5, 0.5)), bursts=(ParticleBurst(0.0, count),),
        lifetime=0.55, lifetime_range=ScalarRange(0.3, 0.55), size=0.008,
        size_range=ScalarRange(0.003, 0.010), speed=0.35, speed_range=ScalarRange(0.2, 0.5),
        spread=360.0, colour="#ffd27a", blend_mode=ParticleBlendMode.ADDITIVE,
        lifetime_style=ParticleLifetimeStyle(size=(ScalarLifetimeStop(0.0, 1.0), ScalarLifetimeStop(1.0, 0.0)), opacity=(ScalarLifetimeStop(0.0, 1.0), ScalarLifetimeStop(1.0, 0.0))),
    )


def radial_burst(*, seed: int = 53, count: int = 48) -> ParticleSystem:
    """Return a larger, medium-lived radial burst from the canvas centre."""
    return ParticleSystem(
        seed=seed, emitter=CircleEmitter(Point(0.5, 0.5), 0.0, 0.0), bursts=(ParticleBurst(0.0, count),),
        lifetime=1.2, size=0.012, size_range=ScalarRange(0.006, 0.016),
        speed=0.3, speed_range=ScalarRange(0.18, 0.42), spread=360.0,
        colour="#8fd8ff", blend_mode=ParticleBlendMode.ADDITIVE,
    )
