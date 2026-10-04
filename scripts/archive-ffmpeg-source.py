#!/usr/bin/env python3
"""Preserve the actual bundled FFmpeg sources for a release wheel, without host dumps."""

import argparse
import json
from pathlib import Path
import shlex
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def git(source: Path, *args: str) -> bytes:
    return subprocess.check_output(
        ["git", "-c", "safe.directory=*", "-C", str(source), *args]
    )


def archive(source: Path, output: Path) -> None:
    revision = git(source, "rev-parse", "HEAD").decode().strip()
    tracked = git(source, "ls-files", "-z").decode().split("\0")
    config = (source / "ffbuild/config.mak").read_text()
    configuration = next(
        line.split("=", 1)[1]
        for line in config.splitlines()
        if line.startswith("FFMPEG_CONFIGURATION=")
    )
    # Prefixes, compiler/tool paths and extra search paths are local build details.
    options = shlex.split(configuration)
    portable = [
        option
        for option in options
        if not any(
            option.startswith(prefix)
            for prefix in (
                "--prefix=",
                "--cc=",
                "--cxx=",
                "--ld=",
                "--ar=",
                "--nm=",
                "--strip=",
                "--extra-cflags=",
                "--extra-ldflags=",
                "--sysroot=",
                "--cross-prefix=",
            )
        )
    ]
    for option in ("--disable-gpl", "--disable-nonfree", "--disable-version3"):
        if option not in portable:
            raise ValueError(
                f"unexpected FFmpeg licensing configuration: missing {option}"
            )
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        for name in tracked:
            if name:
                bundle.write(source / name, "ffmpeg/" + name)
        bundle.writestr(
            "CHANGES.diff", git(source, "diff", "--no-ext-diff", "HEAD", "--")
        )
        bundle.writestr(
            "BUILD.json",
            json.dumps(
                {
                    "upstream": "https://github.com/FFmpeg/FFmpeg",
                    "revision": revision,
                    "configure_options": portable,
                },
                indent=2,
            )
            + "\n",
        )
        bundle.write(ROOT / "vendor/ffmpeg-sys-next/build.rs", "vestra-build.rs")
        bundle.write(ROOT / "vendor/ffmpeg-sys-next/Cargo.toml", "bindings-Cargo.toml")
        bundle.writestr(
            "README.txt",
            """Corresponding FFmpeg source for the accompanying Vestra wheel.
FFmpeg is LGPL 2.1 or later; see ffmpeg/COPYING.LGPLv2.1 and ffmpeg/LICENSE.md.
The ffmpeg/ tree already contains the modifications shown in CHANGES.diff.
BUILD.json records the exact upstream revision and portable configure options.
Local prefix/compiler/search paths have been omitted; select your own tools.
vestra-build.rs is the wheel's build recipe, including target-specific flags.

To modify/rebuild: unpack this archive, edit ffmpeg/, then run ./configure with
BUILD.json's options and a local --prefix, followed by make and make install.
Download/unpack the matching Vestra source distribution from PyPI or GitHub.
For a modified bundled build, adapt fetch() in vendor/ffmpeg-sys-next/build.rs
to copy your modified ffmpeg/ source into source() instead of cloning upstream,
then rebuild Vestra with Maturin. The remaining build recipe configures and
links those libraries. Use a clean Cargo target directory for the rebuild.
The source distribution includes Cargo.lock and the native module's source.
The runtime ffmpeg/ffprobe executables are installed separately.
""",
        )
    print(f"Archived corresponding FFmpeg source: {output.name} ({revision})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = args.source
    if source is None:
        candidates = list((ROOT / "target").glob("**/out/ffmpeg-*/ffbuild/config.mak"))
        if len(candidates) != 1:
            parser.error(
                f"expected exactly one FFmpeg build source, found {len(candidates)}"
            )
        source = candidates[0].parents[1]
    archive(source, args.output)


if __name__ == "__main__":
    main()
