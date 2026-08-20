# Particle system source

`ParticleSystem(*, emitter=PointEmitter(), rate=0, bursts=(), seed=0,
lifetime=1, size=1, colour="#ffffff", opacity=1, ... )` wraps the typed
particle definition. Its canonical tag is `particle_system`.

The emitter is `PointEmitter`, `RectangleEmitter`, or `CircleEmitter`. Emitter
positions and extents are normalized source space. `rate` is continuous
emission; each `ParticleBurst(time, count)` is an instantaneous emission.
`seed` is an unsigned deterministic authored seed. `lifetime`, size, speed,
rotation, angular velocity, direction/spread, acceleration, primitive
(`disc` or `square`), blend mode (`normal` or `additive`), and optional ranges
define motion and appearance. Equal `ScalarRange` endpoints are fixed values;
random samples otherwise use the half-open interval `[min, max)`.

`ParticleLifetimeStyle` animates size, opacity, and colour over normalized
particle age. `ParticleAudioReactive` can bind size, opacity, and intensity to
scalar properties/signals. That modulation is sampled at the current project
time and never changes historical spawn or motion reconstruction.

Validation checks finite/ranged values, emitter geometry, lifetime stops,
colours, and seed bounds. CPU and WGPU have particle rendering paths; exact
backend parity is not fully verified. Particle systems are not direct transform
or transition endpoints, but they may live in a nested composition.
