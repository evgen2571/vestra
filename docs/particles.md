# ParticleSystem authoring

`ParticleSystem` is a renderer-independent procedural visual source. It is
authored as a normal Python object, serialized into the project schema, and
evaluated by the existing CPU renderer.

```python
from vestra import FrameRate
from vestra.authoring import ParticleSystem, PointEmitter, ProjectBuilder

project = ProjectBuilder(width=64, height=64, frame_rate=FrameRate(30, 1),
                         output_path="particles.mp4", duration=1.0)
project.add_particle_system_clip(
    particle_system=ParticleSystem(emitter=PointEmitter(), rate=20.0,
                                    lifetime=1.0, size=0.02),
    start=0, duration=1.0, layer=1,
)
```

Coordinates are normalized canvas coordinates: `(0, 0)` is the top-left and
`(1, 1)` is the bottom-right. Finite values outside this range are useful for
emitters just outside the visible canvas. Disc size is normalized diameter;
square size is normalized side length. CPU rasterization scales either by
`size * min(width, height)`.

Point emitters sample one position. Rectangle emitters sample uniformly over
an area. Circle emitters sample uniformly by annulus area; equal inner and
outer radii form an exact ring. Emission can be continuous (`rate`) or use
timestamped `ParticleBurst` values.

Directions use screen coordinates: 0° is right, 90° is down, 180° is left,
and 270° is up. `spread` is symmetric around the base direction; 360° means
a full circle. `ScalarRange` makes randomized lifetime, size, speed, rotation,
and angular velocity explicit and deterministic.

Particles have stable identities derived from the system seed and their
spawn identity. The same project, seed, and timestamp produce the same state
regardless of frame render order. Vestra is an offline random-access renderer,
not a persistent game-engine simulation. Particle lifetimes are sampled in
seconds, use the half-open interval `[spawn, spawn + lifetime)`, and lifetime
style stops use normalized particle lifetime from 0 to 1—not project seconds.

`ParticleBlendMode` controls overlap within the particle source. The clip's
ordinary `blend_mode` is separate: it composites the completed particle layer
with other clips.

Lifetime styling supports size, opacity, and colour tint curves. Safe
audio-reactive appearance supports only size, opacity, and intensity, using
the existing scalar property/audio signal authoring. Audio-reactive emission,
burst count, lifetime, speed, direction, and acceleration are intentionally
unsupported because they would change particle history and trajectory.

There are three time domains: normalized particle lifetime for lifetime
curves, the ParticleSystem clip-local timeline for authored scalar animation,
and absolute project time for audio signal sampling.

Resource limits bound live particles per system and aggregate simultaneous
live particles. Lifetime ranges are considered conservatively at their
maximum lifetime. CPU and WGPU ParticleSystem rendering use the same particle
state and blend semantics. Normal particles rasterize natively into a
premultiplied temporary and resolve to Vestra's straight-alpha source format
before effects. Additive uses the exact CPU source rasterizer as a WGPU
compatibility path because saturated straight-alpha Additive is not expressible
by the current portable fixed-function blend state. Effects and outer
composition remain on WGPU. WGPU performance validation requires real hardware.

The curated helpers `ambient_stars`, `snow`, `embers`, `sparks`, and
`radial_burst` return ordinary, inspectable `ParticleSystem` objects. They use
fixed default seeds and accept a small number of useful knobs. Returned systems
are mutable, so they can be customized directly:

```python
system = sparks(seed=42)
system.opacity = 0.7
```

Audio-reactive appearance requires authored audio material: master audio signal
modifiers need an audio asset, track, and clip in the project timeline. The
audio-reactive example includes this wiring. Particle systems also use the
ordinary clip effect pipeline; see `examples/particles/sparks-bloom.json` for a
particle source followed by Bloom.
