# Particle system source

```python
ParticleSystem(definition=None, *, emitter=PointEmitter(), rate=0.0, bursts=(), seed=0,
               lifetime=1.0, lifetime_range=None, size=1.0, size_range=None,
               colour="#ffffff", opacity=1.0, initial_velocity=Point(0, 0),
               speed=0.0, speed_range=None, direction=0.0, spread=0.0,
               acceleration=Point(0, 0), rotation=0.0, rotation_range=None,
               angular_velocity=0.0, angular_velocity_range=None,
               primitive=ParticlePrimitive.DISC, blend_mode=ParticleBlendMode.NORMAL,
               lifetime_style=None, audio_reactive=None)
```

Pass either an advanced authoring `ParticleSystem` as `definition`, or configure the keyword fields. `PointEmitter(position=Point(0.5, 0.5))`, `RectangleEmitter(center, size)`, and `CircleEmitter(center, inner_radius=0.0, outer_radius=0.5)` are the emitter forms. Emitter points are normalized source space. Circle radii and rate are non-negative; `inner_radius <= outer_radius`.

| Fields | Default and rule |
| --- | --- |
| `bursts` | Tuple/iterable of `ParticleBurst(time, count)`; time and count are non-negative. |
| `seed` | `0`; deterministic unsigned authored seed. |
| `lifetime`, `size`, `speed`, `spread` | `1.0`, `1.0`, `0.0`, `0.0`; lifetime is positive; size and speed are non-negative; spread is degrees in the inclusive range `0..=360`. |
| `lifetime_range`, `size_range`, `speed_range`, `rotation_range`, `angular_velocity_range` | Optional `ScalarRange(minimum, maximum)` with finite `minimum <= maximum`. Equal bounds are fixed; sampling otherwise uses `[min, max)`. |
| `colour`, `opacity` | `"#ffffff"`, `1.0`; color is canonical and opacity is 0 through 1. |
| motion | `initial_velocity`/`acceleration` are `Point`; direction, rotation, and angular velocity are finite degrees. The canonical fields are `direction_degrees`, `rotation_degrees`, and `angular_velocity_degrees`. |
| `primitive`, `blend_mode` | `DISC`/`SQUARE`; `NORMAL`/`ADDITIVE`. |
| `lifetime_style` | Optional `ParticleLifetimeStyle(size=(), opacity=(), colour=())`; typed stops use normalized age `t` in 0 through 1 and must be strictly increasing. |
| `audio_reactive` | Optional `ParticleAudioReactive(size=None, opacity=None, intensity=None)` with bindable scalar properties. It affects current appearance, never historical spawn/motion reconstruction. |

The canonical tag is `particle_system`. CPU and WGPU have render paths, but full backend parity is not verified. Particle systems can live in nested compositions but are not direct transform or transition endpoints.
