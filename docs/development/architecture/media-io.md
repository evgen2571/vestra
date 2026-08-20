# Media I/O

`vestra-media` owns FFmpeg/FFprobe availability and probing, video/audio decode, master-audio analysis, audio graph compilation, frame sinks, encoder finalization, temporary output, publication, and cleanup. The SDK asks this boundary to validate assets and output paths during preflight, then supplies rendered frames and audio plans during rendering.

Finalization and publication are distinct failure boundaries. A completed encoder alone does not mean the requested output was published.
