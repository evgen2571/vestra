# Video Source

`Video` is a visual source backed by the native FFmpeg media foundation. Its
audio is not imported automatically.

```python
from vestra import Project, Video

project = Project(size=(1280, 720), fps=30, duration=9)
video = Video("assets/clip.mp4")
project.root.add(video, start=0)
project.root.add(
    video,
    start=5,
    source_start=2,
    duration=4,
    playback_rate=0.5,
)
```

When `duration` is omitted, the layer uses the remaining normalized media
duration: `(video_duration - source_start) / playback_rate`. `source_start` is
in normalized source seconds and `playback_rate` must be finite and positive.
Frame selection is timestamp-based, so variable-frame-rate media uses the
latest presentation frame at or before the requested source time. Reverse
playback, time remapping, interpolation, and automatic Video audio remain
unsupported.

Video uses the Image sizing and crop model and the ordinary layer transform,
opacity, blend, effect, group, and transition paths. It presents the coded
orientation RGBA raster: rotation metadata is preserved but not applied, and
sample-aspect-ratio metadata remains metadata-only.
