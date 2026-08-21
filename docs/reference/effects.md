# Effects

Visual effects are ordered canonical objects with `id`, `type` and descriptor parameters. Python classes in `vestra.effects` lower to them. The Rust descriptor catalog supplies canonical validation and generated-schema branches. No visual-effect constructor has an implicit parameter default unless the table says so: required values must be supplied. A scalar property accepts a number or `ScalarProperty` and may use keyframes/signal bindings; a plain track accepts keyframes but not signal bindings.

Layer presentation uses two intrinsic effect stages:

```text
source sizing/crop → pre-transform effects → layer transform
→ post-transform effects → owned masks → Track Matte → opacity/blend
```

Motion Tile is the only pre-transform effect. Other effects execute after the
layer transform in declaration order. The public effect list remains one
ordered list; the compiler routes each canonical effect to its intrinsic
stage. Global post-effects remain post-composition.

| Python class / canonical type | Required parameters and limits | Scope |
| --- | --- | --- |
| `Brightness`, `Contrast`, `Saturation` | `amount`: finite scalar. | clip/global |
| `Tint` | `colour`, `amount`: 0..1 scalar. | clip/global |
| `GaussianBlur` | `radius`: 0..32 px scalar. | clip/global |
| `MotionTile` | `output_width_percent`, `output_height_percent`: 100..800% scalars; `tile_center`: normalized point; `mirror_edges`: boolean. | clip/global, pre-transform |
| `DirectionalBlur` | `radius`: 0..32 px scalar, `angle_degrees`: finite scalar. | clip/global |
| `ZoomBlur` | `radius`: 0..32 px scalar, `samples`: integer 2..32, `anchor`: normalized point, `direction=ZoomBlurDirection.CENTERED` (`centered`, `inward`, `outward`). | clip/global |
| `RadialBlur` | `amount`: 0..32 px scalar, `center`: normalized point. | clip/global |
| `Glow` | `threshold`: 0..1, `radius`: 0..32 px, `intensity`: 0..4 scalars, `colour`. | clip/global |
| `Bloom` | `threshold`: 0..1, `radius`: 0..32 px, `intensity`: 0..4 scalars. | clip/global |
| `ChromaticAberration` | `amount`: 0..32 px scalar, `angle_degrees`: finite scalar. | clip/global |
| `Vignette` | `amount`: 0..1 scalar, `radius`: 0..2 scalar, `softness`: plain track in `(0, 2]`, `colour`. | clip/global |
| `Sharpen` | `amount`: 0..4 scalar, `radius`: 0..16 px scalar. | clip/global |
| `ColorAdjust` | `exposure`: -8..8 scalar, `gamma`: `(0, 8]` scalar, `black_point`: plain track `[0, 1)`, `white_point`: plain track `(0, 1]`. | clip/global |
| `CameraShake` | `position_amount`, `rotation_degrees`, `scale_amount`: non-negative scalars; `frequency`: positive scalar; `seed`: unsigned integer; `attack`: non-negative seconds; `decay`: positive seconds; `active_interval=ActiveInterval()`. | clip only |
| `MotionBlur` | `intensity`: non-negative scalar, `shutter_angle`: 0..360 degrees scalar, `max_radius`: 0..32 px scalar, `samples`: integer 2..32. | clip only |

Numbers must be finite. Stack order is execution order within each intrinsic
stage. Motion Tile repeats the already-sized/cropped source over a virtual
sampling region while the final output remains the normal composition extent;
it does not allocate an expanded full-frame image. `mirror_edges=False` repeats
directly and `mirror_edges=True` alternates mirrored tiles, including negative
virtual coordinates. Its RGBA samples are copied together, so transparent
source pixels remain transparent. CPU and WGPU use the same coordinate model.
The canonical form uses each class's snake-case type such as `gaussian_blur`;
color values are canonical `#RRGGBB`/`#RRGGBBAA`.
