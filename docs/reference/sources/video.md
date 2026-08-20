# Video source

`Video(path, *, sizing=None, crop=None)` accepts a non-empty string path.
Its canonical tag is `video`; path resolution, dimensions, duration, and decode
readiness belong to preflight/preparation. `sizing` and `crop` have the same
contract as [Image](image.md): `original`, `fit`, `cover`, or `Sizing` for the
additional scale/stretch forms, plus a mutable `CropProperty`.

The visual layer supplies project-timeline `start` and `duration`. Canonical
video clips additionally carry source trim and playback settings where authored
through the canonical builder. Source-media duration and coded dimensions are
not constructor arguments and must be probed. Layer transform, animation,
effects, and transitions apply as for images; video is a direct transition
endpoint.

CPU and WGPU render paths consume decoded video frames, subject to FFmpeg,
codec, and media availability. The public limitation is operational: a valid
path can still fail preflight for an unsupported stream or missing media tool.
