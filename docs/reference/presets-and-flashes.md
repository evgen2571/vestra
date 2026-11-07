# Presets and flashes

Presets expand a layer's motion intent; flashes add root-owned color overlays.
See the [guide](../guides/python/transitions-flashes-and-presets.md) for examples.

## Layer presets

Presets are convenience values, not a separate rendering primitive. Current
kinds are `slow_drift`, `zoom_punch`, `impact`, `heavy_impact` and `focus_reveal`.

```text title="Preset value signature"
Preset(kind, intensity=1.0, start=0.0, duration=None, seed=None)
```

| Parameter | Default | Contract |
| --- | --- | --- |
| `intensity` | `1.0` | Finite value in `[0, 2]`. |
| `start` | `0.0` | Non-negative layer-local seconds. |
| `duration` | `None` | Optional positive seconds. |
| `seed` | `None` | Required unsigned 64-bit integer for `impact`/`heavy_impact`; rejected for other kinds. |

Invalid kinds, values and seed combinations fail during construction.
The high-level `PresetCollection` permits at most one preset on a layer and
currently accepts only Image layers. Clear the current preset before adding
another. Presets lower to canonical image-clip preset values.

## Root flashes

```text title="Flash value signature"
Flash(start, duration, colour, opacity=1.0, fade_in=0.0, fade_out=0.0, layer=0, id=None)
```

| Parameter | Contract |
| --- | --- |
| `start`, `duration` | Project-timeline seconds; non-negative start and positive duration. |
| `colour` | Canonical color string, authoring color or solid-color source. |
| `opacity` | Finite value in `[0, 1]`; default `1.0`. |
| `fade_in`, `fade_out` | Non-negative seconds; their sum must fit inside duration. Default `0.0`. |
| `layer` | Integer stacking order; default `0`. |
| `id` | Optional non-empty identifier; the collection can assign one. |

Python exposes `Flash` and `FlashCollection`; canonical JSON stores flashes
under `visual`. Construction checks timing, opacity, color, IDs and fade fit;
canonical validation also checks the placed overlay in its project context.
