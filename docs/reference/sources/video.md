# Video source

`Video` draws decoded frames from a file. Construct the source with sizing and
crop, then choose its source offset, playback rate and timeline span when placing
it. See [sources and layers](../../guides/python/sources-and-layers.md) for placement examples.

## Source construction

```text title="Source signature (presentation keywords omitted)"
Video(path: str | PathLike[str], *, sizing: Sizing | Literal["original", "fit", "cover"] | None = None,
      crop: Crop | CropProperty | None = None)
```

| Parameter | Default | Meaning |
| --- | --- | --- |
| `path` | Required | Non-empty `str` or `PathLike[str]` naming a video file. |
| `sizing` | `None` | Original intrinsic dimensions; an override uses the [Image sizing contract](image.md#parameters-and-sizing), including `Sizing.scale` and `Sizing.stretch`. |
| `crop` | `None` | Full source region; an override uses [normalized source coordinates](image.md#crop-coordinates). |

The constructor also accepts [presentation transform defaults](../../guides/python/sources-and-layers.md),
copied into each placement and individually overridable by `add()`.
Source trim and playback rate belong to placement, not the constructor.

## Placement and time domains

```text title="Placement timing signature (presentation keywords omitted)"
composition.add(video, *, start=0, duration=None, source_start=0.0,
                playback_rate=1.0, z=0, visible=True, opacity=1.0,
                id=None, name=None, blend_mode=BlendMode.NORMAL)
```

| Parameter | Default | Units and constraints |
| --- | --- | --- |
| `start` | `0` | Non-negative seconds on the owning composition's timeline. |
| `duration` | `None` | Positive owning-composition seconds; omission derives the usable media span. |
| `source_start` | `0.0` | Non-negative source-media seconds. |
| `playback_rate` | `1.0` | Finite positive multiplier on source time. |

When `duration=None`, `add()` **probes the file immediately** and derives
`(media_duration - source_start) / playback_rate`. The source offset must precede
the media end. Supply an explicit duration to avoid this authoring-time probe;
it does not bypass later media validation.

Layers own placement timing, transforms, animation, effects and transitions.
Video is a direct transform and transition endpoint on CPU and WGPU after media
preparation. See [timeline and time](../../concepts/timeline-and-time.md) for nested clocks.

## Paths, preparation and errors

| Stage | Failure |
| --- | --- |
| Construction | Invalid/empty path, invalid sizing or invalid crop type. Crop bounds follow the [Image validation rules](image.md#validation-and-errors). |
| Placement with omitted duration | Probe failure or an offset at/after the media end. |
| Semantic validation | Invalid placement timing or canonical source/asset relationships. |
| Preflight/preparation | Missing, corrupt or unsupported media, unavailable tools/decoder, or a source span beyond the available video. |

Relative paths resolve against the project's base directory. Coded duration and
intrinsic dimensions are media facts gathered by probing/preparation, rather than
values stored in the source object. CPU and WGPU consume the prepared decoded frames.

## Canonical JSON

The `video` source contains an `asset` reference. The surrounding clip carries
`start`, `duration`, optional `source_start`/`playback_rate`, sizing, crop and
presentation. See [project format](../project-format.md) for asset and clip fields.
