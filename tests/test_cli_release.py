"""CLI release archives are installable, checksummed, and checked after extraction."""

import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import zipfile

import pytest

ROOT = Path(__file__).resolve().parents[1]


def script(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.mark.parametrize(
    ("target", "suffix", "executable"),
    [
        ("x86_64-unknown-linux-gnu", ".tar.gz", "ve"),
        ("aarch64-unknown-linux-gnu", ".tar.gz", "ve"),
        ("x86_64-apple-darwin", ".tar.gz", "ve"),
        ("aarch64-apple-darwin", ".tar.gz", "ve"),
        ("x86_64-pc-windows-msvc", ".zip", "ve.exe"),
    ],
)
def test_package_keeps_binary_licenses_and_checksum(
    tmp_path, target, suffix, executable
):
    binary = tmp_path / executable
    binary.write_bytes(b"built CLI fixture\n")
    output = script("package-cli").package(binary, target, "1.2.3", tmp_path / "dist")
    stem = f"ve-1.2.3-{target}"
    assert output.name == stem + suffix
    checksum = output.with_name(output.name + ".sha256").read_text()
    assert (
        checksum
        == f"{hashlib.sha256(output.read_bytes()).hexdigest()}  {output.name}\n"
    )
    destination = tmp_path / "extracted"
    script("verify-cli").extract(output, destination)
    assert (destination / stem / executable).read_bytes() == b"built CLI fixture\n"
    assert (destination / stem / "LICENSE").is_file()
    assert (destination / stem / "THIRD_PARTY_NOTICES.md").is_file()
    assert (destination / stem / "licenses/FFmpeg-LGPL-2.1.txt").is_file()
    assert (destination / stem / "licenses/DejaVuSans.txt").is_file()
    assert "ffprobe" in (destination / stem / "README.txt").read_text()
    if suffix == ".tar.gz":
        assert (destination / stem / executable).stat().st_mode & 0o111
        with tarfile.open(output) as bundle:
            assert all(member.uid == member.gid == 0 for member in bundle)
        with tarfile.open(output) as bundle:
            assert all(member.uname == member.gname == "" for member in bundle)


def test_package_rejects_missing_binary_without_leaving_an_archive(tmp_path):
    with pytest.raises(FileNotFoundError):
        script("package-cli").package(
            tmp_path / "missing", "x86_64-unknown-linux-gnu", "1.2.3", tmp_path / "dist"
        )
    assert not list(tmp_path.rglob("*.tar.gz"))


def test_package_command_reads_workspace_version(tmp_path):
    binary = tmp_path / "ve"
    binary.write_bytes(b"fixture")
    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts/package-cli.py"),
            "--binary",
            str(binary),
            "--target",
            "x86_64-unknown-linux-gnu",
            "--output-dir",
            str(tmp_path / "dist"),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    import tomllib

    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"][
        "version"
    ]
    assert (
        tmp_path / "dist" / f"ve-{version}-x86_64-unknown-linux-gnu.tar.gz"
    ).is_file()


def test_checksum_rejects_corruption_and_wrong_filename(tmp_path):
    archive = tmp_path / "ve.tar.gz"
    archive.write_bytes(b"archive bytes")
    checksum = archive.with_name(archive.name + ".sha256")
    checksum.write_text(f"{'0' * 64}  {archive.name}\n")
    verifier = script("verify-cli")
    with pytest.raises(ValueError, match="checksum"):
        verifier.verify_checksum(archive)
    checksum.write_text(
        f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  other.tar.gz\n"
    )
    with pytest.raises(ValueError, match="checksum"):
        verifier.verify_checksum(archive)


@pytest.mark.parametrize("kind", ["zip", "tar", "symlink"])
def test_extract_rejects_path_traversal_and_links(tmp_path, kind):
    archive = tmp_path / ("bad.zip" if kind == "zip" else "bad.tar.gz")
    if kind == "zip":
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr("../escaped", "bad")
    else:
        with tarfile.open(archive, "w:gz") as bundle:
            member = tarfile.TarInfo("ve/link" if kind == "symlink" else "../escaped")
            if kind == "symlink":
                member.type = tarfile.SYMTYPE
                member.linkname = "../../escaped"
                bundle.addfile(member)
            else:
                member.size = 3
                bundle.addfile(member, io.BytesIO(b"bad"))
    with pytest.raises(ValueError, match="unsafe archive"):
        script("verify-cli").extract(archive, tmp_path / "extracted")
    assert not (tmp_path / "escaped").exists()


def probe():
    return {
        "streams": [
            {
                "codec_name": "h264",
                "pix_fmt": "yuv420p",
                "width": 32,
                "height": 32,
                "avg_frame_rate": "4/1",
                "nb_read_frames": "4",
                "duration": "1.000000",
            }
        ],
        "frames": [
            {"best_effort_timestamp_time": f"{index / 4:.6f}"} for index in range(4)
        ],
    }


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("nb_read_frames", "3"),
        ("codec_name", "vp9"),
        ("width", 64),
        ("duration", "0.75"),
    ],
)
def test_probe_rejects_incomplete_or_wrong_output(field, value):
    data = probe()
    data["streams"][0][field] = value
    with pytest.raises(ValueError, match="rendered video"):
        script("verify-cli").verify_probe(data)


def test_probe_checks_presentation_timestamps():
    verifier = script("verify-cli")
    data = probe()
    verifier.verify_probe(data)
    data["frames"][-1]["best_effort_timestamp_time"] = "0.5"
    with pytest.raises(ValueError, match="timestamps"):
        verifier.verify_probe(data)


def test_smoke_rejects_binary_version_before_render(tmp_path, monkeypatch):
    verifier = script("verify-cli")
    monkeypatch.setattr(
        subprocess,
        "run",
        lambda *args, **kwargs: subprocess.CompletedProcess(args, 0, "ve 0.0.0\n", ""),
    )
    with pytest.raises(ValueError, match="version"):
        verifier.smoke(tmp_path / "ve", tmp_path, "1.2.3")


def test_smoke_preserves_cli_failure_diagnostics(tmp_path, monkeypatch):
    verifier = script("verify-cli")

    def fail(*args, **kwargs):
        raise subprocess.CalledProcessError(
            3, args[0], output='{"errors":["invalid project"]}'
        )

    monkeypatch.setattr(subprocess, "run", fail)
    with pytest.raises(RuntimeError, match="invalid project"):
        verifier.smoke(tmp_path / "ve", tmp_path, "1.2.3")


def test_smoke_fixture_needs_no_repository_assets(tmp_path):
    project = script("verify-cli").smoke_project()
    schema = json.loads((ROOT / "schemas/project.schema.json").read_text())
    for clip in project["visual"]["clips"]:
        assert set(schema["$defs"]["clip"]["required"]) <= clip.keys()
    assert not project["assets"]
    assert project["output"]["audio"] is False
    assert {clip["source"]["type"] for clip in project["visual"]["clips"]} == {
        "solid_color"
    }
    # JSON round-trip catches accidental paths or other values the CLI cannot consume.
    (tmp_path / "project.json").write_text(json.dumps(project))
