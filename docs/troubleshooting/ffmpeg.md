# FFmpeg and media problems

Start with the failing operation. Python wheel users can diagnose executable
and media errors without installing a compiler. Build-tool failures apply only
to the [source-build path](../development/source-builds.md).

<span id="ffmpeg-or-ffprobe-is-missing-at-runtime"></span>

## `ffmpeg` is missing at runtime

**Check.** Read the selected executable path in the diagnostic. Python uses the
executable installed through `imageio-ffmpeg`; the separate native CLI and Rust
SDK use `ffmpeg` on `PATH`. Check whether `IMAGEIO_FFMPEG_EXE` overrides Python's
selection. A bare system `ffmpeg` is never a silent Python fallback.

**Fix.** For Python, reinstall `imageio-ffmpeg` in the same environment as
Vestra, or remove an invalid override to restore the packaged default:

```bash
uv pip install --reinstall imageio-ffmpeg
```

In a pip environment, use `python -m pip install --force-reinstall imageio-ffmpeg`
with the interpreter that runs the script. For the CLI or developer media tools,
install FFmpeg and make it available on `PATH` in their shell.

**Confirm.** Rerun the failing render. For the CLI, first run `ffmpeg -version`
in the same shell. If selecting your own executable, follow the
[override contract](../reference/environment-variables.md#choose-a-different-ffmpeg-executable),
including the required encoders and filters. Prepare again after changing it;
prepared projects keep the executable selected during preparation.

Media probing uses bundled FFmpeg libraries and does not require `ffprobe`.
Install `ffprobe` only for optional developer checks that verify encoded output
independently.

### NixOS

**Check.** Distinguish an extension import error from an executable launch error.
The native extension and packaged FFmpeg executable have separate runtime
requirements. A missing ELF interpreter or shared library while importing
`_native` belongs to the extension.

**Fix.** Use the repository's [Nix source-build environment](../development/source-builds.md#with-nix-linux-and-macos)
when the extension cannot load. If only the packaged executable cannot run,
select Nix's FFmpeg through `IMAGEIO_FFMPEG_EXE`. That override cannot fix an
extension import failure.

**Confirm.** Check `import vestra` first, then rerun the render. This verifies
both boundaries rather than treating an import as an encoding test.

## An existing asset does not decode

**Check.** Read the diagnostic code, asset path and selected stream. A present
file can still fail probing, stream selection or decoder setup. In Python,
check paths relative to `project.base_directory`. With a CLI project, run:

```bash
ve validate project.json --format json
```

**Fix.** Correct the asset path or replace/re-encode a corrupt or unsupported
stream. Preserve the original file until you have a reproducible failure;
video and audio decode errors can appear only when that resource is prepared.

**Confirm.** Run preparation or validation again, then render the interval
that previously failed. Passing preflight alone does not exercise every decoded
video frame.

## Encoding or final output fails

**Check.** Find whether the error names a frame write, encoder finalization or
publication. Check the output format, selected encoder, destination permissions,
overwrite policy and free disk space. A completed frame loop can still fail
while finalizing the container.

**Fix.** Use a supported [output format](../reference/render-outputs.md), a
writable destination and an executable with the required encoder. Do not delete
or replace an existing destination until you know whether publication completed.

**Confirm.** Rerun to a fresh destination and open the published file. Keep the
native encoder error and temporary/final paths if it still fails; see
[render failure evidence](rendering.md#report-an-unresolved-render-failure).

## Build cannot find FFmpeg

This section applies to a source build, not an installed wheel.

**Check.** Read the first compiler/configure failure. `vestra-media` builds and
statically links FFmpeg libraries through `ffmpeg-next`. Installing the `ffmpeg`
executable alone does not provide its build tools.

**Fix.** Follow [source-build prerequisites](../development/source-builds.md).
The build needs a C compiler, `make`, `git`, NASM and initial network access
for the FFmpeg source fetch. The Nix shell supplies these tools. A local patch
removes the dependency's host-specific `-march=native` flag for portable wheels.

**Confirm.** Re-run the build from the configured development environment.
If it still fails, retain the first configure/compiler error, operating system,
architecture, tool versions and exact build command.
