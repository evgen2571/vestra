# FFmpeg and media problems

## Build cannot find FFmpeg

`vestra-media` builds and statically links the FFmpeg libraries it uses through
`ffmpeg-next`. Source builds need a C compiler, `make`, `git`, `nasm`, and
network access for the first FFmpeg source fetch. A local patch removes the
dependency's host-specific `-march=native` compiler flag for portable wheels.
The repository Nix development shell supplies the build tools.

<span id="ffmpeg-or-ffprobe-is-missing-at-runtime"></span>

## `ffmpeg` is missing at runtime

Python installs FFmpeg through `imageio-ffmpeg` and uses its packaged executable.
If Vestra reports that FFmpeg is unavailable, reinstall that dependency in the
same environment. The diagnostic includes the selected executable path. Check
whether `IMAGEIO_FFMPEG_EXE` selects a missing or incompatible build; unset it
to restore the packaged default. A bare system `ffmpeg` is never a silent fallback.

To select a different build, set `IMAGEIO_FFMPEG_EXE` to its executable path
before preparation or rendering. It must provide `libx264`, AAC and the audio
filters needed by your project. Keep the path, `-version` output and native
error in a bug report. Prepared projects keep the executable selected during
preparation; prepare again after changing the override.

The separate native CLI and developer media tools still require `ffmpeg` on
`PATH`. Run `ffmpeg -version` in the shell running those tools.

### NixOS

The imageio-ffmpeg 0.6.0 Linux x86_64 executable is static and was verified on
NixOS 26.05. An installed Vestra wheel built on NixOS was also checked with
`PATH` empty and `LD_LIBRARY_PATH` unset. This does not prove that a published
manylinux extension loads on every NixOS configuration: native dynamic runtime
dependencies remain separate from the FFmpeg executable. If your installation
reports a missing ELF interpreter or shared library, use the repository's Nix
development environment for a source build. You can select Nix's FFmpeg with
`IMAGEIO_FFMPEG_EXE` if the packaged executable cannot run on your system.

Media probing uses bundled FFmpeg libraries and does not require `ffprobe`.
Some optional developer checks use `ffprobe` to verify output independently;
install it when running those checks.

## An existing asset does not decode

Run `ve validate project.json` to reach validation-target preflight. A present path can still fail probing, stream selection or decoder setup because of an unsupported/corrupt media stream. Video decode failures and audio decode failures may appear only after the relevant resource is prepared. Check the diagnostic code, asset path and selected stream rather than replacing the asset blindly.

## Encoding or final output fails

An encoder can reject a selected container/output combination, and a completed frame loop can still fail during encoder finalization or final publication. Check destination permissions, overwrite policy and free space. Preserve the diagnostic's temporary and final paths. Do not delete or replace the destination until you know whether publication completed. See [rendering problems](rendering.md) for failure stages.
