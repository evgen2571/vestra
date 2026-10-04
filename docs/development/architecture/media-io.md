# Media I/O

`vestra-media` owns every transition between Vestra values and media bytes. `probe` checks image/video/audio metadata needed by preflight; `video` supplies decoder sessions; `audio_graph` prepares and executes tracks, effects and master analysis; `sink` accepts rendered frames and encodes them; `output` finalizes and publishes the result.

The crate uses `ffmpeg-next` for native FFmpeg format and software-scaling work. It also has operational checks for `ffmpeg` and `ffprobe` executables where a workflow needs them. Those are separate dependencies. The native dependency builds bundled FFmpeg libraries from source; wheel users do not need the C/Rust build toolchain. A source build can fail when its compiler, NASM or source fetch is unavailable, while a runtime operation can fail when an executable or codec is unavailable.

Preflight resolves the project-relative asset paths once, probes relevant media, and records dimensions/durations for plan compilation. Preparation decodes static resources, creates video decoder sessions and produces master-audio analysis when visuals need it. During rendering, the SDK supplies rendered frames to the frame sink while the audio graph produces the audio execution output. The media layer, not a renderer, owns muxing and encoded-output lifetime.

Frame writing, encoder finalization and publication are deliberately separate. The sink writes a temporary target, finalizes the container, then `output` publishes it at the requested destination subject to overwrite policy. Each stage can fail and cleanup preserves the original destination where possible. Callers should use the stage, temporary path and destination in `RenderFailureContext` rather than assuming that "encoding completed" means the output exists.

For frame selection, caching and concurrency contracts, see the [media pipeline](../media-pipeline.md).
