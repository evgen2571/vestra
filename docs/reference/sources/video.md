# Video source

```python
Video(path: str | PathLike[str], *, sizing: Sizing | Literal["original", "fit", "cover"] | None = None,
      crop: Crop | CropProperty | None = None)
```

`path` must be a non-empty text path. `sizing` and `crop` have the exact [Image](image.md) contract, including `Sizing.scale`/`Sizing.stretch` and normalized `Crop`. The high-level constructor does not accept source trim or playback rate: those belong to the layer placement.

```python
composition.add(video, *, start=0, duration=None, source_start=0.0,
                playback_rate=1.0, z=0, visible=True, opacity=1.0,
                id=None, name=None, blend_mode=BlendMode.NORMAL)
```

`start` and `duration` are project or owning-composition local seconds. `source_start` is source-media seconds and may be zero; `playback_rate` is a finite positive multiplier. If `duration=None`, a `Video` layer probes the file immediately to derive the available media duration after `source_start`, divided by playback rate. Provide `duration` to avoid that authoring-time probe. Canonical video clips carry this placement timing; coded duration and dimensions remain media facts gathered by preflight/preparation.

Video is a direct transform and transition endpoint. CPU and WGPU consume decoded frames after media preparation. A valid path can still fail preflight for unsupported/corrupt media, missing tools or an unavailable decoder.
