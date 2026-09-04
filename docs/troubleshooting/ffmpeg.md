# FFmpeg and media problems

## Build cannot find FFmpeg

`vestra-media` links native FFmpeg through `ffmpeg-next` and enables its bundled
FFmpeg build. A plain Cargo build therefore does not require `libavcodec.pc`,
`libavformat.pc`, `libavutil.pc`, or `libswscale.pc`; it does require a C
compiler, `make`, `git`, `nasm`, and network access for the first build. Remove
stale build artifacts and retry with `cargo clean && cargo build` if a previous
checkout was built with the system-library configuration. The repository Nix
development shell supplies the required tools.

## `ffmpeg` or `ffprobe` is missing at runtime

Some operational checks/workflows require `ffmpeg` and `ffprobe` on `PATH`; that is separate from native linking. Confirm with `command -v ffmpeg` and `command -v ffprobe`, then run their `-version` commands in the same shell that runs Vestra. Keep the executable path and version in a bug report.

## An existing asset does not decode

Run `ve validate project.json` to reach validation-target preflight. A present path can still fail probing, stream selection or decoder setup because of an unsupported/corrupt media stream. Video decode failures and audio decode failures may appear only after the relevant resource is prepared. Check the diagnostic code, asset path and selected stream rather than replacing the asset blindly.

## Encoding or final output fails

An encoder can reject a selected container/output combination, and a completed frame loop can still fail during encoder finalization or final publication. Check destination permissions, overwrite policy and free space. Preserve the diagnostic's temporary and final paths. Do not delete or replace the destination until you know whether publication completed. See [rendering problems](rendering.md) for failure stages.
