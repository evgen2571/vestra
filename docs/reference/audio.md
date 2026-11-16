---
description: Audio timeline time domains, effect scopes, output policy and crossfade constraints.
---

# Audio

Use tracks to place source excerpts on the project timeline, and clip, track
or master effects to choose the processing scope. For an existing `project`
and a source file containing at least five seconds of audio:

```python
music = project.audio.track("music", gain=0.9)
clip = music.add("assets/music.wav", start=2, trim_start=1, trim_end=5)
```

The selected source seconds 1–5 play from project second 2. See
[add audio](../guides/python/audio.md) for a complete rendering task and
[audio and signals](../concepts/audio-and-signals.md) for analysis.

## Python placement

```text title="Signature summary (gain, fades, mute and IDs listed below)"
project.audio.track(identifier=None, *, id=None, mute=False, gain=1.0)
track.add(path, *, start=0.0, trim_start=0.0, trim_end=None, ...)
```

| Parameter | Default | Units or constraints |
| --- | --- | --- |
| Track `identifier` / `id` | Generated ID. | Supply at most one; non-empty and unique in the audio timeline. |
| Clip `path` | Required. | Audio path; relative paths use the project base directory. |
| Clip `id` | Generated ID. | Non-empty and unique across the audio timeline's clips. |
| `start` | `0.0` | Non-negative project seconds. |
| `trim_start`, `trim_end` | `0.0`, `None` | Source-media seconds; an explicit end must follow the start. Omitted end uses decoded source duration. |
| Track/clip `gain`, `mute` | `1.0`, `False` | Non-negative finite multiplier; boolean mute. |
| `fade_in`, `fade_out` | `0.0` | Non-negative clip-relative durations in seconds. |
| `fade_in_curve`, `fade_out_curve` | `AudioFadeCurve.LINEAR` | `LINEAR` or `EQUAL_POWER`. |

Construction/assignment rejects invalid values and duplicate IDs. Canonical
validation checks timeline and effect relationships; preflight resolves media
and duration. Valid authoring values alone do not establish media readiness.

## Canonical JSON

The optional canonical `audio` object has ordered tracks and master effects.
Python paths register asset references during lowering; canonical clips use
asset IDs rather than Python source paths.

| Object | Contents and defaults |
| --- | --- |
| Audio timeline | Ordered `tracks` and master `effects`. |
| Track | `id`, clips, `mute=false`, `gain=1` and track effects. |
| Clip | ID, asset/source, `start`, `trim_start`, optional `trim_end`, `gain=1`, `mute=false`, fades, optional gain automation and clip effects. |

See [project format](project-format.md#audio) for the canonical object fields.

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

Preflight measures the first audio stream's decoded samples using the bundled
FFmpeg libraries; `ffprobe` is not required. An omitted `trim_end` uses that
duration, excluding other streams and timestamp offsets. Probing reads the full
source, so long audio takes longer to validate or prepare. Codec padding can
make the result differ slightly from container metadata. Assets sharing a resolved
path are probed once per preflight. One-shot render cancellation is checked
between native packet/frame operations. See
[audio probing](../development/architecture/media-io.md#audio-duration) for
failure cases and native path restrictions.

## Effects

| Type / Python constructor | Parameters | Scope and duration |
| --- | --- | --- |
| `parametric_eq` / `ParametricEq` | Required `frequency_hz` >0 through 24000 Hz, `gain_db` -24..24 dB, `q` >0 through 100 | clip, track, master; preserves duration |
| `bass_boost` / `BassBoost` | `gain_db=6` from 0..24 dB, `frequency_hz=100` from 20..250 Hz | clip, track, master; preserves duration |
| `playback_speed` / `PlaybackSpeed` | Required `rate` 0.25..4; dimensionless speed multiplier | clip only; transforms duration |

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
