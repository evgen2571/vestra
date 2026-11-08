#!/usr/bin/env python3
"""Verify the release inventory and bind binary artifacts to corresponding source."""

import argparse
import hashlib
import json
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import urlopen
import zipfile

PLATFORMS = {
    "x86_64-unknown-linux-gnu": "manylinux_2_28_x86_64",
    "aarch64-unknown-linux-gnu": "manylinux_2_28_aarch64",
    "x86_64-apple-darwin": "macosx_11_0_x86_64",
    "aarch64-apple-darwin": "macosx_11_0_arm64",
    "x86_64-pc-windows-msvc": "win_amd64",
}


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_manifest(directory: Path, version: str, revision: str) -> None:
    expected = {f"vestra-{version}.tar.gz"}
    for target, platform in PLATFORMS.items():
        wheels = list(directory.glob(f"vestra-{version}-cp311-abi3-*{platform}*.whl"))
        if len(wheels) != 1:
            raise ValueError(f"expected one abi3 wheel for {target}, got {len(wheels)}")
        suffix = ".zip" if "windows" in target else ".tar.gz"
        cli = directory / f"ve-{version}-{target}{suffix}"
        checksum = cli.with_name(cli.name + ".sha256")
        if not cli.is_file() or not checksum.is_file():
            raise ValueError(f"missing CLI archive/checksum for {target}")
        if checksum.read_text().strip() != f"{sha256(cli)}  {cli.name}":
            raise ValueError(f"CLI checksum mismatch for {target}")
        expected.update((wheels[0].name, cli.name, checksum.name))
        for kind, binary in [("wheel", wheels[0]), ("cli", cli)]:
            source = directory / f"ffmpeg-{kind}-{target}.zip"
            with zipfile.ZipFile(source) as archive:
                build = json.loads(archive.read("BUILD.json"))
            if build["vestra_revision"] != revision or build["artifacts"] != [
                {"name": binary.name, "sha256": sha256(binary)}
            ]:
                raise ValueError(
                    f"FFmpeg source association mismatch for {binary.name}"
                )
            expected.add(source.name)
    actual = {path.name for path in directory.iterdir() if path.is_file()}
    actual -= {"release-manifest.json", "SHA256SUMS"}
    if actual != expected:
        raise ValueError(
            f"release inventory mismatch: missing={expected - actual}, extra={actual - expected}"
        )
    records = [
        {"name": name, "sha256": sha256(directory / name)} for name in sorted(expected)
    ]
    manifest = directory / "release-manifest.json"
    manifest.write_text(
        json.dumps(
            {"version": version, "revision": revision, "artifacts": records}, indent=2
        )
        + "\n"
    )
    checksums = records + [{"name": manifest.name, "sha256": sha256(manifest)}]
    (directory / "SHA256SUMS").write_text(
        "".join(f"{item['sha256']}  {item['name']}\n" for item in checksums)
    )
    print(f"Validated {len(records)} artifacts from {revision}")


def verify_existing(files: list[Path], published: dict) -> None:
    existing = {
        item["filename"]: item["digests"]["sha256"] for item in published["urls"]
    }
    for path in files:
        if path.name in existing and existing[path.name] != sha256(path):
            raise ValueError(
                f"PyPI already contains a different {path.name}; never replace published artifacts"
            )


def verify_staged(directory: Path, staged: Path) -> None:
    for existing in staged.iterdir():
        candidate = directory / existing.name
        if candidate.is_file() and sha256(existing) != sha256(candidate):
            raise ValueError(
                f"staged release already contains a different {existing.name}; reuse the original run artifacts"
            )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--verify-pypi", action="store_true")
    parser.add_argument("--compare-directory", type=Path)
    args = parser.parse_args()
    write_manifest(args.directory, args.version, args.revision)
    if args.compare_directory:
        verify_staged(args.directory, args.compare_directory)
    if args.verify_pypi:
        try:
            with urlopen(
                f"https://pypi.org/pypi/vestra/{args.version}/json", timeout=30
            ) as response:
                published = json.load(response)
        except HTTPError as error:
            if error.code != 404:
                raise
        else:
            files = list(args.directory.glob("*.whl")) + list(
                args.directory.glob("vestra-*.tar.gz")
            )
            verify_existing(files, published)


if __name__ == "__main__":
    main()
