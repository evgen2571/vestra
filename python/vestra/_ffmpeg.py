"""Select the Python runtime's FFmpeg without changing process-wide PATH."""

import os
from pathlib import Path
import shutil

import imageio_ffmpeg  # type: ignore[import-untyped]


def resolve_ffmpeg() -> str:
    if override := os.environ.get("IMAGEIO_FFMPEG_EXE"):
        return str(Path(shutil.which(override) or override).resolve())

    # Each supported dependency wheel contains one binary. Its public resolver
    # executes FFmpeg and can silently fall back to a system installation.
    directory = Path(imageio_ffmpeg.__file__).resolve().parent / "binaries"
    binaries = list(directory.glob("ffmpeg-*"))
    # Defer missing/ambiguous-package failures to native diagnostics so frame-only
    # preparation continues to work without an executable.
    return str(binaries[0] if len(binaries) == 1 else directory / "ffmpeg")
