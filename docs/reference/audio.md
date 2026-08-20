# Audio

The canonical model has an optional `audio` timeline. It contains master
`effects` and ordered `tracks`. A track has `id`, `mute`, `gain`, `effects`, and
`clips`. A clip identifies an audio asset and has `start`, `trim_start`,
optional `trim_end`, `gain`, optional `gain_automation`, `fade_in`, `fade_out`,
fade curves, `mute`, and effects.

## Time domains

| Field | Domain |
| --- | --- |
| `start` | Project timeline seconds. |
| `trim_start`, `trim_end` | Source-media positions in seconds. |
| `fade_in`, `fade_out` | Durations relative to the selected clip. |
| Gain automation `time` | Clip-local seconds. |
| Crossfade overlap | Project timeline overlap derived from clip placement and source trims. |

Gain defaults to `1.0`; `mute` defaults to `false`; fades default to `0.0`;
fade curves default to `linear`. Gain interpolation is `linear` or `hold`.
`trim_end`, when present, must be after `trim_start`. Audio validation checks
asset identity, timing, automation duration and ordering, effect scope, and
media availability during preflight.

## Effects and exposure

The current audio effect catalog is `parametric_eq`, `bass_boost`, and
`playback_speed`. `ParametricEq` has `frequency_hz`, `gain_db`, and `q`.
`BassBoost` has `gain_db` and `frequency_hz`, with descriptor defaults.
`PlaybackSpeed` has `rate`. Effects have scope rules. The Python API exposes
the same concepts through `AudioEffectStack` and the `ParametricEq`,
`BassBoost`, and `PlaybackSpeed` classes.

Audio analysis is prepared before signal-driven rendering. `output.audio`
controls whether authored audio is published. The high-level Python `None`
policy follows whether the project contains clips; explicit `True` or `False`
overrides it.
