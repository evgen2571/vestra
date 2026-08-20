# Video source

`vestra.sources.Video(path, sizing=None, crop=None)` references a video media
asset. The canonical source tag is `video`. The layer controls timeline start,
duration, source timing, playback rate, transform, animation, effects, and
transition participation. Sizing and crop use the same public concepts as an
Image source.

Video duration, dimensions, and decode details are resolved during preflight or
preparation through the media boundary. The path must identify a readable
video. Missing media, unsupported codecs, and unavailable FFmpeg are
environment failures. Video is current supported functionality in the Python,
canonical, CPU, and WGPU paths, subject to the installed media environment.
