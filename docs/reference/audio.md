# Audio

The optional canonical `audio` object has ordered `tracks` and master `effects`. A track has `id`, `mute=false`, `gain=1`, effects, and clips. A clip has an id, asset/source, `start`, `trim_start`, optional `trim_end`, `gain=1`, `mute=false`, fades, optional gain automation, and effects.

| Field | Time domain |
| --- | --- |
| `start` | Project timeline seconds. |
| `trim_start`, `trim_end` | Source-media seconds. |
| `fade_in`, `fade_out` | Durations relative to the selected clip. |
| Gain-automation keyframe `time` | Clip-local seconds. |

Fades default to 0 and their curves to `linear`. Gain automation uses `linear` or `hold` interpolation. `trim_end` must follow `trim_start`; validation also checks ids, assets, timing, automation order/duration, effect scope, and media availability during preflight.

| Type | Parameters | Scope and duration |
| --- | --- | --- |
| `parametric_eq` | `frequency_hz` >0 through 24000, `gain_db` -24..24, `q` >0 through 100 | clip, track, master; preserves duration |
| `bass_boost` | `gain_db=6` from 0..24, `frequency_hz=100` from 20..250 | clip, track, master; preserves duration |
| `playback_speed` | `rate` 0.25..4 | clip only; transforms duration |

Effects execute in stack order. High-level Python exposes `ParametricEq`, `BassBoost`, `PlaybackSpeed`, and `AudioEffectStack`; advanced canonical authoring uses the same descriptor catalog. Audio analysis is prepared before signal-driven visuals. `output.audio` controls publication; the high-level `None` policy follows whether clips exist, while an explicit boolean overrides it.
