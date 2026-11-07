# Particle system source

`ParticleSystem` generates deterministic particles from an emitter, seed and
emission schedule. Use it for procedural bursts or continuous emission. A default
system emits nothing: `rate=0.0` and `bursts=()`.

## API signature

```text title="Source signature (presentation keywords omitted)"
ParticleSystem(definition=None, *, emitter=PointEmitter(), rate=0.0, bursts=(), seed=0,
               lifetime=1.0, lifetime_range=None, size=1.0, size_range=None,
               colour="#ffffff", opacity=1.0, initial_velocity=Point(0, 0),
               speed=0.0, speed_range=None, direction=0.0, spread=0.0,
               acceleration=Point(0, 0), rotation=0.0, rotation_range=None,
               angular_velocity=0.0, angular_velocity_range=None,
               primitive=ParticlePrimitive.DISC, blend_mode=ParticleBlendMode.NORMAL,
               lifetime_style=None, audio_reactive=None)
```

Pass an advanced authoring `ParticleSystem` as `definition`, or configure the
keyword fields. Supplying `definition` selects that definition; the particle
configuration keywords are not merged into it.

## Emitters and coordinates

| Emitter signature | Coordinates and constraints |
| --- | --- |
| `PointEmitter(position=Point(0.5, 0.5))` | One normalized source-space position. |
| `RectangleEmitter(center, size)` | Normalized source-space `Point` values; semantic validation requires non-negative size components. |
| `CircleEmitter(center, inner_radius=0.0, outer_radius=0.5)` | Normalized source-space center and radii; radii are non-negative with `inner_radius <= outer_radius`. |

Emitter coordinates are not shape pixels. Velocity and speed use normalized
canvas units per second; acceleration uses those units per second squared.
Direction uses +X right and +Y down.

## Emission and lifetime

| Fields | Default | Units and rule |
| --- | --- | --- |
| `rate` | `0.0` | Finite non-negative particles per second. |
| `bursts` | `()` | Iterable of `ParticleBurst(time, count)`; non-negative layer-local seconds and non-negative integer count. |
| `seed` | `0` | Deterministic unsigned authored seed. |
| `lifetime` | `1.0` | Positive seconds. |
| `lifetime_range` | `None` | Optional `ScalarRange` of lifetimes in seconds. |

## Motion

| Fields | Default | Units and rule |
| --- | --- | --- |
| `initial_velocity`, `acceleration` | `Point(0, 0)` | Finite normalized canvas units per second / per second squared. |
| `speed`, `speed_range` | `0.0`, `None` | Non-negative normalized canvas units per second; optional range replaces fixed speed. |
| `direction`, `spread` | `0.0`, `0.0` | Finite degrees; spread is in `[0, 360]`. |
| `rotation`, `rotation_range` | `0.0`, `None` | Finite degrees; optional range replaces fixed orientation. |
| `angular_velocity`, `angular_velocity_range` | `0.0`, `None` | Finite degrees per second; optional range replaces fixed angular velocity. |

## Appearance

| Fields | Default | Units and rule |
| --- | --- | --- |
| `size`, `size_range` | `1.0`, `None` | Non-negative fraction of the shorter canvas dimension; disc diameter or square side length. |
| `colour`, `opacity` | `"#ffffff"`, `1.0` | Canonical color and opacity in `[0, 1]`. |
| `primitive`, `blend_mode` | `DISC`, `NORMAL` | `DISC`/`SQUARE`; `NORMAL`/`ADDITIVE`. |
| `lifetime_style` | `None` | `ParticleLifetimeStyle(size=(), opacity=(), colour=())`; typed stops use normalized age `t` in `[0, 1]`, strictly increasing per curve. |
| `audio_reactive` | `None` | `ParticleAudioReactive(size=None, opacity=None, intensity=None)`; bindable scalar appearance properties. |

## Lifetime, validation and presentation

Ranges use `ScalarRange(minimum, maximum)` in the corresponding property's units.
Equal bounds are fixed; otherwise sampling uses `[minimum, maximum)`.
The Python range object checks finite ordered endpoints. Semantic validation also
requires positive, timeline-representable lifetime bounds and non-negative size
and speed bounds. Invalid emitter/configuration types, base values and lifetime
stop ordering fail during authoring; canonical validation checks resource and
simulation limits as well.

Emission and particle age follow layer-local time. `audio_reactive` uses prepared
audio analysis for appearance only; it does not reconstruct past emission or
motion from audio. Without audio-reactive bindings, this source needs no media
asset preparation.

The canonical tag is `particle_system`, with `emitter`, `emission` and `particle`
objects. Python `direction`, `spread`, `rotation` and `angular_velocity` lower to
`direction_degrees`, `direction_spread_degrees`, `rotation_degrees` and
`angular_velocity_degrees` inside `particle`.

Particle systems can live in nested compositions. They are not direct canonical
transform or transition endpoints; high-level Python adapts presentation when
needed. CPU and WGPU have render paths, but full backend parity is not verified.
See [feature support](../feature-support.md) and [signals](../signals.md).

`rotation` above is particle orientation. Use `rotation_degrees` for layer
presentation. The constructor also accepts [presentation transform defaults](../../guides/python/sources-and-layers.md),
which are copied into each placement and can be individually overridden by `add()`.
