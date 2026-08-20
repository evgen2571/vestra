# Signals

Signals are immutable scalar values derived from prepared master-audio analysis.
Use `project.audio.signal` in high-level Python. Each constructor or transform
returns a new `ScalarSignal`; it does not mutate the input.

| Python form | Canonical feature | Contract |
| --- | --- | --- |
| `signal.rms()` | `rms` | Master RMS value. |
| `signal.peak()` | `peak` | Master peak value. |
| `signal.band(min_hz, max_hz)` | `band_energy` | Master band energy in Hz. `0 <= min_hz < max_hz <= 24000`. `band_energy` is an alias. |

The canonical signal has `source.type` set to `audio`, `source.tap` set to
`master`, and `source.feature` set to an `rms`, `peak`, or `band_energy`
feature object.
The optional `transforms` array preserves call order.

| Method | Canonical transform | Validation |
| --- | --- | --- |
| `gain(value)` | `gain` | Finite scalar. |
| `remap(input_min, input_max, output_start, output_end)` | `remap` | All values finite and `input_min < input_max`. Keyword-pair form is `remap(input=(a, b), output=(c, d))`. |
| `clamp(minimum, maximum)` | `clamp` | Finite values and `minimum <= maximum`. |
| `envelope(attack, release)` | `envelope` | Non-negative elapsed seconds. |
| `response_curve(x1, y1, x2, y2)` | `response_curve` | Finite values and `0 <= x1 <= x2 <= 1`; x is input position and y is output value. |

Signals are evaluated in project-timeline time from master audio. Preparation
must obtain usable audio analysis; an audio-less project cannot satisfy a signal
binding. Bind a signal to a `BindableScalarProperty` with
`property.bind(signal, operation="replace")`. The operations are `replace`, `add`,
and `multiply`: replace supplies the signal value, add adds it to the property
value, and multiply multiplies the property value by it. The target property
still enforces its own range after evaluation.

Signals are available in the high-level Python layer and lower to canonical
JSON. They are not standalone runtime SDK objects. See the [audio reference](audio.md)
and [audio-reactivity guide](../guides/python/signals-and-audio-reactivity.md).
