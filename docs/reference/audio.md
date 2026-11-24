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
| `fade_in`, `fade_out` | Durations within the clip after playback speed. |
| Gain-automation keyframe `time` | Clip-local seconds after playback speed. |

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
| `playback_speed` / `PlaybackSpeed` | Required `rate` from 0.25 through 4. Boolean `link_pitch` defaults to `false` in JSON and `False` in Python. | Clip only. Divides the selected duration by the rate. |
| `room_reverb` / `RoomReverb` | Required `decay_seconds` from 0.1 through 2 seconds and `mix` from 0 through 1. | Clip, track, master. Preserves duration. |
| `filtered_delay` / `FilteredDelay` | Required `delay_ms` from 1 through 1000 ms, integer `repeats` from 1 through 8, `mix` from 0 through 1 and `cutoff_hz` from 20 Hz up to (exclusive) 24000 Hz. | Clip, track, master. Preserves duration. |
| `beat_repeat` / `BeatRepeat` | Required `slice_start` and `output_start` in non-negative seconds, `slice_duration` from 0.01 through 1 second, and integer `repeats` from 1 through 8 total plays. | Clip only. Preserves duration. |
| `filter_sweep` / `FilterSweep` | Required `mode` (`low_pass`, `high_pass`, or `band_pass`) and 1–256 `keyframes` with `time` in seconds and `frequency_hz` from 20 through 20,000 Hz. | Clip only. Preserves duration. |
| `pitch_shift` / `PitchShift` | Required `semitones` from -12 through +12, including fractional values. Zero passes the input through unchanged. | Clip only. Preserves tempo and duration. |
| `tape_stop` / `TapeStop` | Required `start` in non-negative seconds and `duration` from 0.05 through 2 seconds. | Clip only. Preserves duration. |

`PlaybackSpeed(rate=1.10)` preserves pitch. With `link_pitch=True`, tempo and
pitch both scale by the rate, so a 440 Hz tone moves to about 484 Hz at 1.10×.
Either mode uses the planned sample count at the 48 kHz mixer rate. Fades
and gain automation use the processed clip's clock.
Rust callers constructing `AudioEffect::PlaybackSpeed` set `link_pitch` explicitly.

Both spatial effects use `mix=0` for dry passthrough and `mix=1` for wet
only. Room reverb adds short stereo reflections that decay over the
requested response length. Filtered delay places the first repeat one delay
interval later and spaces the remaining repeats by the same interval,
rounded to 48 kHz samples. Each repeat has half the gain of the previous
one. Only the repeats pass through the low-pass filter. For an eighth-note
delay, divide 30,000 by the **remixed** tempo in beats per minute.

The four remix effects use the processed clip-local clock at their position in
the stack. Time zero is the first sample after trimming and preceding effects,
including any `PlaybackSpeed`. For example, if playback speed comes first, a
two-second source clip at 2× supplies a one-second span to the next effect.

- **Beat repeat** reads a stereo slice from its input and replaces the window
  starting at `output_start` with the authored number of plays. Both the slice
  and the replacement window must fit in the clip. Start times and slice length
  round to the nearest 48 kHz sample. The replacement window is the rounded
  slice length times `repeats`. Short fades smooth the joins. At your chosen BPM,
  use `30 / BPM` seconds for an eighth note or `15 / BPM` for a sixteenth note.
- **Filter sweep** uses a two-pole filter with fixed Q of 0.707. Set cutoff in
  low-pass or high-pass mode and center frequency in band-pass mode. In
  Python, use `AudioFilterKeyframe(time, frequency_hz)`. In JSON, use objects
  with those two fields. Keyframes start at zero, strictly increase at 48 kHz
  resolution, and can reach the end of the clip. Frequency interpolates linearly
  in Hz, updates every 2.5 ms, and holds after the last keyframe.
- **Pitch shift** resamples the audio and applies inverse tempo compensation.
  A +2 semitone shift moves 440 Hz to approximately 494 Hz. Individual attacks
  can shift by a few tens of milliseconds. The portable processor shifts
  formants with pitch. FFmpeg's optional Rubber Band filter offers formant
  preservation in builds that include it. Vestra currently exposes the portable
  processor for both full mixes and isolated vocals.
- **Tape stop** slows speed and pitch together over the authored interval and
  fades to silence. The interval must fit in the clip. The clip stays silent
  after the stop. Start the next phrase on a separate clip to bring the mix back
  on the downbeat.

Effects run in authored order within each stack. Each clip is trimmed and
processed before clip gain, gain automation, fades and timeline placement.
Each track processes its summed clips before track gain. The master stack
processes the sum of all tracks. Clip-level tails stay within the processed
clip span. Track and master tails can ring past the source clips within the
resolved project span. To hear more of the tail, leave space in the project
after the source ends. The effects never add duration on their own.

Add these values to an `AudioEffectStack` in high-level Python. The advanced
`AudioEffectCollection` exposes typed methods with the same parameter names,
including `add_beat_repeat`, `add_filter_sweep`, `add_pitch_shift`, and
`add_tape_stop`. The generic `add_effect` accepts the canonical tag and its
parameters. See the [chorus edit recipe](../guides/python/audio.md#edit-into-a-chorus)
for an example that uses all four edits.

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
