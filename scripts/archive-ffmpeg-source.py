#!/usr/bin/env python3
"""Preserve the bundled FFmpeg sources for an associated release artifact."""

import argparse
import hashlib
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


def archive(source: Path, output: Path, *, artifacts: list[Path] | None = None) -> None:
    revision = git(source, "rev-parse", "HEAD").decode().strip()
    tracked = git(source, "ls-files", "-z").decode().split("\0")
    config = (source / "ffbuild/config.mak").read_text()
    configuration = next(
        line.split("=", 1)[1]
        for line in config.splitlines()
        if line.startswith("FFMPEG_CONFIGURATION=")
    )
    for name in ("COPYING.LGPLv2.1", "LICENSE.md"):
        if name not in tracked or not (source / name).is_file():
            raise ValueError(f"missing FFmpeg license: {name}")
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
                "--sysroot=",
                "--cross-prefix=",
            )
        )
    ]
    for index, option in enumerate(portable):
        if option.startswith(("--extra-cflags=", "--extra-ldflags=")):
            key, value = option.split("=", 1)
            flags = []
            values = iter(shlex.split(value))
            for flag in values:
                if flag in ("-I", "-L", "-F", "-isystem", "-isysroot", "--sysroot"):
                    next(values, None)
                elif not flag.startswith(("-I", "-L", "-F", "-isystem", "-isysroot")):
                    if "/" not in flag and "\\" not in flag:
                        flags.append(flag)
            portable[index] = key + "=" + shlex.join(flags)
    for component in ("gpl", "nonfree", "version3"):
        if (
            f"--disable-{component}" not in portable
            or f"--enable-{component}" in portable
        ):
            raise ValueError(
                f"unexpected FFmpeg licensing configuration: requires --disable-{component}"
            )
    associated = []
    for artifact in artifacts or []:
        if artifact.resolve() == output.resolve():
            raise ValueError(
                "source archive output must not overwrite its binary artifact"
            )
        with artifact.open("rb") as binary:
            associated.append(
                {
                    "name": artifact.name,
                    "sha256": hashlib.file_digest(binary, "sha256").hexdigest(),
                }
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
                    "vestra_revision": git(ROOT, "rev-parse", "HEAD").decode().strip(),
                    "artifacts": associated,
                    "configure_options": portable,
                },
                indent=2,
            )
            + "\n",
        )
        bundle.write(ROOT / "vendor/ffmpeg-sys-next/build.rs", "vestra-build.rs")
        bundle.write(ROOT / "vendor/ffmpeg-sys-next/Cargo.toml", "bindings-Cargo.toml")
        bundle.write(ROOT / "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_NOTICES.md")
        bundle.writestr(
            "README.txt",
            """Corresponding FFmpeg source for the Vestra artifacts identified in BUILD.json.
FFmpeg is LGPL 2.1 or later; see ffmpeg/COPYING.LGPLv2.1 and ffmpeg/LICENSE.md.
The ffmpeg/ tree already contains the modifications shown in CHANGES.diff.
BUILD.json records the exact upstream/Vestra revisions, artifact filenames and
SHA-256 digests, and portable configure options.
Local prefix/compiler/search paths have been omitted; select your own tools.
vestra-build.rs is the bundled-library build recipe, including target-specific flags.

To modify/rebuild: unpack this archive, edit ffmpeg/, then run ./configure with
BUILD.json's options and a local --prefix, followed by make and make install.
For CLI rebuilds, download/unpack the complete matching Vestra source revision
from GitHub. Python-module rebuilds can also use the matching PyPI source
distribution, which excludes the CLI crate. BUILD.json identifies the revision.
For a modified bundled build, adapt fetch() in vendor/ffmpeg-sys-next/build.rs
to copy your modified ffmpeg/ source into source() instead of cloning upstream,
then rebuild the Python module with Maturin or the CLI with
cargo build --locked --release -p vestra-cli. The remaining build recipe configures and
links those libraries. On Windows, skip the two patch_windows_* calls when
reusing this already patched source tree. Use a clean Cargo target directory
for the rebuild.
The source distribution includes Cargo.lock and the native module's source.
The runtime ffmpeg/ffprobe executables are installed separately.
This archive supplies library source material; it does not certify that all
static-linking LGPL relinking obligations have been met. Review LGPL 2.1 section 6.
""",
        )
    print(f"Archived corresponding FFmpeg source: {output.name} ({revision})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    location = parser.add_mutually_exclusive_group()
    location.add_argument("--source", type=Path)
    location.add_argument("--search-root", type=Path, default=ROOT / "target")
    parser.add_argument("--artifact", type=Path, action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = args.source
    if source is None:
        candidates = list(args.search_root.glob("**/out/ffmpeg-*/ffbuild/config.mak"))
        if len(candidates) != 1:
            parser.error(
                f"expected exactly one FFmpeg build source, found {len(candidates)}"
            )
        source = candidates[0].parents[1]
    archive(source, args.output, artifacts=args.artifact)


if __name__ == "__main__":
    main()
