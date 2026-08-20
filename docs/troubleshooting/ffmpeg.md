# FFmpeg and media problems

Runtime rendering needs `ffmpeg` and `ffprobe` on `PATH`. Building the native media crate also needs its FFmpeg development libraries and `pkg-config`; these are different requirements. Use the repository Nix environment when available.

An existing asset can still fail because the stream/codec cannot be decoded, probing fails, or the selected output/encoder cannot be opened. A finished frame loop can still fail during encoder finalization or final output publication. Keep the temporary/final output path from the diagnostic when reporting a failure.
