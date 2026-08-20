# Effects

Visual effects attach to a clip or to the global visual effect scope allowed by
the descriptor. Python exposes them from `vestra.effects`; canonical JSON uses
an effect object with `type`, non-empty `id`, and descriptor parameters. Effect
order is the order in the owning effect stack.

The current visual catalog is `brightness`, `contrast`, `saturation`, `tint`,
`gaussian_blur`, `directional_blur`, `zoom_blur`, `glow`, `bloom`,
`chromatic_aberration`, `vignette`, `sharpen`, `color_adjust`, `camera_shake`,
and `motion_blur`. The descriptor catalog in
`vestra-core/src/effect_definition.rs` supplies names, parameter kinds,
defaults, ranges, enum values, and scope metadata used by schema generation.

| Effect | Parameters |
| --- | --- |
| Brightness, Contrast, Saturation | `amount` |
| Tint | `colour`, `amount` |
| GaussianBlur | `radius` |
| DirectionalBlur | `radius`, `angle_degrees` |
| ZoomBlur | `radius`, `samples`, `anchor`, `direction` |
| Glow | `threshold`, `radius`, `intensity`, `colour` |
| Bloom | `threshold`, `radius`, `intensity` |
| ChromaticAberration | `amount`, `angle_degrees` |
| Vignette | `amount`, `radius`, `softness`, `colour` |
| Sharpen | `amount`, `radius` |
| ColorAdjust | `exposure`, `gamma`, `black_point`, `white_point` |
| CameraShake | active interval, `position_amount`, `rotation_degrees`, `scale_amount`, `frequency`, `seed`, `attack`, `decay` |
| MotionBlur | `intensity`, `shutter_angle`, `max_radius`, `samples` |

Scalar parameters are typed properties and may be animated. Signals can bind
where the owning property supports binding. Validation rejects non-finite
values, invalid ranges, duplicate ids, unsupported scopes, and effect-specific
constraints. The descriptor ranges are:

| Parameter group | Accepted values |
| --- | --- |
| `tint.amount`, `glow.threshold`, `bloom.threshold`, `vignette.amount` | `0..=1` |
| Blur radii, `zoom_blur.radius`, `glow.radius`, `bloom.radius`, `chromatic_aberration.amount`, `motion_blur.max_radius` | `0..=32` |
| `glow.intensity`, `bloom.intensity`, `sharpen.amount` | `0..=4` |
| `vignette.radius` | `0..=2` |
| `sharpen.radius` | `0..=16` |
| `color_adjust.exposure` | `-8..=8` |
| `color_adjust.gamma` | `(0, 8]` |
| Camera shake position/rotation/scale amounts and motion-blur intensity | finite, non-negative |
| Camera shake frequency | finite and positive |
| `motion_blur.shutter_angle` | `0..=360` |
| `zoom_blur.samples`, `motion_blur.samples` | integer `2..=32` |

Unbounded angle and amount values documented as finite accept any finite
number. `zoom_blur.direction` defaults to `centered`; camera-shake `attack`
defaults to `0` and `decay` has a strictly positive authored value when
provided. Other descriptor parameters are required unless the generated schema
records a default. CPU and WGPU support is descriptor-specific and is covered
by renderer tests; an enum name alone is not a parity claim.
