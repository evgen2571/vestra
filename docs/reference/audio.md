---
description: Audio timeline time domains, effect scopes, output policy and crossfade constraints.
---

# Audio

The optional canonical `audio` object has ordered `tracks` and master `effects`.
A track has `id`, `mute=false`, `gain=1`, effects, and clips. A clip has an id,
asset/source, `start`, `trim_start`, optional `trim_end`, `gain=1`, `mute=false`,
fades, optional gain automation, and effects.

For practical setup, see [add audio](../guides/python/audio.md). For analysis
and visual consumers, see [audio and signals](../concepts/audio-and-signals.md).

## Timing and validation

| Field | Time domain |
| --- | --- |
| `start` | Project timeline seconds. |
| `trim_start`, `trim_end` | Source-media seconds. |
| `fade_in`, `fade_out` | Durations relative to the selected clip. |
| Gain-automation keyframe `time` | Clip-local seconds. |

Fades default to 0 and their curves to `linear`. Gain automation uses `linear`
or `hold` interpolation. `trim_end` must follow `trim_start`; validation also
checks ids, assets, timing, automation order/duration, effect scope, and media
availability during preflight.

## Effects

| Type | Parameters | Scope and duration |
| --- | --- | --- |
| `parametric_eq` | `frequency_hz` >0 through 24000, `gain_db` -24..24, `q` >0 through 100 | clip, track, master; preserves duration |
| `bass_boost` | `gain_db=6` from 0..24, `frequency_hz=100` from 20..250 | clip, track, master; preserves duration |
| `playback_speed` | `rate` 0.25..4 | clip only; transforms duration |

Effects execute in stack order. High-level Python exposes `ParametricEq`,
`BassBoost`, `PlaybackSpeed`, and `AudioEffectStack`; advanced canonical
authoring uses the same descriptor catalog.

## Crossfades

```text title="Python API signature"
project.audio.crossfade(outgoing, incoming, *, curve=AudioFadeCurve.EQUAL_POWER)
```

The high-level helper requires two distinct clips owned by the same audio
timeline, an earlier outgoing start, an explicit outgoing `trim_end`, and an
existing positive overlap. The incoming selected clip must be long enough for
the overlap when it has an explicit trim end. Existing fades must either be
unset or match the requested overlap and curve; conflicts raise `AuthoringError`
without overwriting those settings.

The helper computes overlap as
`outgoing.start + outgoing.trim_end - outgoing.trim_start - incoming.start`.
This uses authored trim spans; it does not account for duration changes from
`PlaybackSpeed`. It sets outgoing fade-out and incoming fade-in; it does not
change starts or visual transitions. Curves are `AudioFadeCurve.LINEAR` or
`AudioFadeCurve.EQUAL_POWER`.

## Analysis and output policy

Prepared master audio supplies [scalar signals](signals.md) and
[Spectrum2D](sources/spectrum2d.md), after clip/track/master processing.
Analysis is independent of whether the render publishes audio. Audio media
execution is shared by the visual backends; a GPU render is not evidence of
hardware audio processing or encoding.

Canonical `output.audio` controls publication. High-level Python calls this
`Project(..., output_audio=None)` or `project.output_audio`: `None` follows
whether clips exist, while an explicit boolean overrides it. Removing clips
can remove analysis input; setting `output_audio=False` retains it.
