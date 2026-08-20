# Particle system source

`ParticleSystem` supports point, rectangle, and circle emitters, continuous
`rate`, timed `bursts`, a 64-bit `seed`, lifetime and size ranges, color and
opacity, initial velocity, speed and direction, acceleration, rotation and
angular velocity, `disc` or `square` primitives, and `normal` or `additive`
particle blending. Defaults include rate `0`, lifetime `1`, size `1`, opacity
`1`, speed `0`, a point emitter at `(0.5, 0.5)`, disc primitive, and normal
blend mode.

Particle simulation is reconstructed from source-local/system time. Lifetime
styles can animate size, opacity, and color over normalized particle lifetime.
Audio-reactive appearance can modulate size, opacity, and intensity, but does
not reconstruct spawn or motion history. The canonical tag is
`particle_system`.

Validation checks ranges, emitter geometry, lifetime stops, colors, and seed
values. CPU and WGPU support follows their tested particle paths. Backend
selection does not change the public simulation contract.
