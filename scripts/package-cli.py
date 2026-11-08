#!/usr/bin/env python3
"""Package an independently built ve executable and its redistribution notices."""

import argparse
import hashlib
from pathlib import Path
import tarfile
import tempfile
import tomllib
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
)
README = """Vestra command-line executable

Extract this archive and put ve (ve.exe on Windows) on PATH, or run it by its
full path. On Unix, preserve its executable permission (chmod +x ve if needed).
Run ve --version and ve --help to check the installation. Python is not needed.

Rendering and media inspection require separately installed ffmpeg and ffprobe
executables on PATH. Rendering currently requires an FFmpeg build with libx264.
The executable bundles FFmpeg libraries; it is not fully self-contained.
Use the archive matching your operating system and processor architecture.
See the release installation documentation for supported operating-system versions
and system-library requirements. These archives are not universal binaries.

LICENSE covers Vestra. THIRD_PARTY_NOTICES.md and licenses/ cover bundled code
and fonts. Matching FFmpeg corresponding-source archives accompany the release.
See the release source distribution and maintainer documentation for rebuild
instructions and the status of static-linking/relinking obligations.
"""


def package(binary: Path, target: str, version: str, output_dir: Path) -> Path:
    if target not in TARGETS:
        raise ValueError(f"unsupported CLI target: {target}")
    if not binary.is_file():
        raise FileNotFoundError(f"CLI binary does not exist: {binary}")
    windows = target.endswith("windows-msvc")
    stem = f"ve-{version}-{target}"
    output_dir.mkdir(parents=True, exist_ok=True)
    output = output_dir / (stem + (".zip" if windows else ".tar.gz"))
    with tempfile.TemporaryDirectory(prefix="vestra-cli-package-") as directory:
        root = Path(directory) / stem
        (root / "licenses").mkdir(parents=True)
        executable = root / ("ve.exe" if windows else "ve")
        shutil.copyfile(binary, executable)
        executable.chmod(0o755)
        for name in ("LICENSE", "THIRD_PARTY_NOTICES.md"):
            shutil.copyfile(ROOT / name, root / name)
        for license in sorted((ROOT / "licenses").glob("*.txt")):
            shutil.copyfile(license, root / "licenses" / license.name)
        (root / "README.txt").write_text(README, encoding="utf-8")
        if windows:
            with zipfile.ZipFile(
                output, "w", compression=zipfile.ZIP_DEFLATED
            ) as bundle:
                for path in sorted(root.rglob("*")):
                    if path.is_file():
                        bundle.write(path, path.relative_to(root.parent).as_posix())
        else:

            def portable(info: tarfile.TarInfo) -> tarfile.TarInfo:
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                return info

            with tarfile.open(output, "w:gz") as bundle:
                bundle.add(root, arcname=stem, filter=portable)
    with output.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    output.with_name(output.name + ".sha256").write_text(
        f"{digest}  {output.name}\n", encoding="utf-8"
    )
    return output


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"][
        "version"
    ]
    print(package(args.binary, args.target, version, args.output_dir))


if __name__ == "__main__":
    main()
