# FFmpeg and media problems

## Build cannot find FFmpeg

`vestra-media` links native FFmpeg through `ffmpeg-next`. A build error about `libavcodec.pc`, `libavformat.pc`, `libavutil.pc`, `libswscale.pc` or `pkg-config` means the development headers/libraries are missing from the active build environment. Installing only an `ffmpeg` executable does not fix that. Use the repository Nix development shell where possible, or install the platform's FFmpeg development package and make its `pkg-config` files visible.

## `ffmpeg` or `ffprobe` is missing at runtime

Some operational checks/workflows require `ffmpeg` and `ffprobe` on `PATH`; that is separate from native linking. Confirm with `command -v ffmpeg` and `command -v ffprobe`, then run their `-version` commands in the same shell that runs Vestra. Keep the executable path and version in a bug report.

## An existing asset does not decode

Run `ve validate project.json` to reach validation-target preflight. A present path can still fail probing, stream selection or decoder setup because of an unsupported/corrupt media stream. Video decode failures and audio decode failures may appear only after the relevant resource is prepared. Check the diagnostic code, asset path and selected stream rather than replacing the asset blindly.

## Encoding or final output fails

An encoder can reject a selected container/output combination, and a completed frame loop can still fail during encoder finalization or final publication. Check destination permissions, overwrite policy and free space. Preserve the diagnostic's temporary and final paths. Do not delete or replace the destination until you know whether publication completed. See [rendering problems](rendering.md) for failure stages.
