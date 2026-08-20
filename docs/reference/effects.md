# Effects

Visual effects are ordered canonical objects with `id`, `type`, and descriptor parameters. Python classes in `vestra.effects` lower to that form. The Rust descriptor catalog supplies Python metadata and generated-schema branches. Scalar-property parameters accept keyframes and signal bindings; plain tracks accept keyframes but not signal bindings.

| Canonical type | Parameters | Scope |
| --- | --- | --- |
| `brightness`, `contrast`, `saturation` | `amount`, finite scalar | clip/global |
| `tint` | `colour`, `amount` 0..1 | clip/global |
| `gaussian_blur` | `radius` 0..32 px | clip/global |
| `directional_blur` | `radius` 0..32 px, finite `angle_degrees` | clip/global |
| `zoom_blur` | `radius` 0..32 px, `samples` integer 2..32, normalized `anchor`, `direction` `centered`/`inward`/`outward`, default `centered` | clip/global |
| `glow`, `bloom` | `threshold` 0..1, `radius` 0..32 px, `intensity` 0..4; glow also has `colour` | clip/global |
| `chromatic_aberration` | `amount` 0..32 px, finite `angle_degrees` | clip/global |
| `vignette` | `amount` 0..1, `radius` 0..2, `softness` >0 through 2, `colour` | clip/global |
| `sharpen` | `amount` 0..4, `radius` 0..16 px | clip/global |
| `color_adjust` | `exposure` -8..8, `gamma` >0 through 8, `black_point` 0..<1, `white_point` >0..1 | clip/global |
| `camera_shake` | optional `active_interval`; non-negative position/rotation/scale, positive `frequency`, u64 `seed`, non-negative `attack`, positive `decay` | clip only |
| `motion_blur` | non-negative `intensity`, `shutter_angle` 0..360 degrees, `max_radius` 0..32 px, `samples` integer 2..32 | clip only |

Numbers must be finite. Parameters without a stated default are required. Stack order is execution order. Validation rejects duplicate ids, bad scopes/ranges, malformed tracks, and invalid active intervals. CPU and WGPU implement this catalog through their effect-pass paths; exact parity for a particular effect beyond covered renderer tests is not fully verified.
