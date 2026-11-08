"""Corresponding-source archives keep edits and omit machine paths/build junk."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import zipfile

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/archive-ffmpeg-source.py"
spec = importlib.util.spec_from_file_location("ffmpeg_archive", SCRIPT)
archive = importlib.util.module_from_spec(spec)
spec.loader.exec_module(archive)


def source_tree(tmp_path: Path) -> Path:
    source = tmp_path / "source"
    source.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", str(source)], check=True)
    (source / "configure").write_text("original source\n")
    (source / "COPYING.LGPLv2.1").write_text("LGPL license fixture\n")
    (source / "LICENSE.md").write_text("FFmpeg license fixture\n")
    subprocess.run(["git", "-C", str(source), "add", "."], check=True)
    subprocess.run(
        [
            "git",
            "-C",
            str(source),
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
        check=True,
    )
    (source / "configure").write_text("modified source\n")
    (source / "ffbuild").mkdir()
    (source / "ffbuild/config.mak").write_text(
        "FFMPEG_CONFIGURATION=--prefix=/private/build "
        "--extra-cflags='-I/private/includes -w -mmacosx-version-min=10.13' "
        "--extra-ldflags='-L /private/lib -flto' "
        "--disable-gpl --disable-nonfree --disable-version3 --enable-static\n"
    )
    (source / "generated.o").write_bytes(b"private object data")
    return source


def test_archive_contains_corresponding_source_and_patch_without_machine_metadata(
    tmp_path: Path,
):
    source = source_tree(tmp_path)
    output = tmp_path / "bundle.zip"
    artifact = tmp_path / "ve-linux-x86_64.tar.gz"
    artifact.write_bytes(b"packaged binary\n")
    archive.archive(source, output, artifacts=[artifact])
    with zipfile.ZipFile(output) as bundle:
        assert bundle.read("ffmpeg/configure") == b"modified source\n"
        assert b"+modified source" in bundle.read("CHANGES.diff")
        info = json.loads(bundle.read("BUILD.json"))
        assert len(info["revision"]) == 40
        assert "--enable-static" in info["configure_options"]
        assert (
            "--extra-cflags=-w -mmacosx-version-min=10.13" in info["configure_options"]
        )
        assert "--extra-ldflags=-flto" in info["configure_options"]
        assert b"/private/" not in bundle.read("BUILD.json")
        assert info["artifacts"] == [
            {
                "name": "ve-linux-x86_64.tar.gz",
                "sha256": "520c3ea67573c07eab06b0e9052b88e9a5b1ba273fe13dc0a5b3713389ece401",
            }
        ]
        assert (
            info["vestra_revision"]
            == subprocess.check_output(
                ["git", "-C", str(SCRIPT.parents[1]), "rev-parse", "HEAD"], text=True
            ).strip()
        )
        assert "ffmpeg/generated.o" not in bundle.namelist()
        assert not any(".git/" in name for name in bundle.namelist())
        assert "vestra-build.rs" in bundle.namelist()
        assert bundle.read("ffmpeg/COPYING.LGPLv2.1") == b"LGPL license fixture\n"
        assert "THIRD_PARTY_NOTICES.md" in bundle.namelist()


def test_archive_rejects_unexpected_license_configuration(tmp_path: Path):
    source = source_tree(tmp_path)
    config = source / "ffbuild/config.mak"
    config.write_text(config.read_text().replace("--disable-gpl", "--enable-gpl"))
    with pytest.raises(ValueError, match="licensing configuration"):
        archive.archive(source, tmp_path / "bundle.zip")


def test_archive_rejects_missing_license_text(tmp_path: Path):
    source = source_tree(tmp_path)
    (source / "COPYING.LGPLv2.1").unlink()
    with pytest.raises(ValueError, match="missing FFmpeg license"):
        archive.archive(source, tmp_path / "bundle.zip")


@pytest.mark.parametrize("component", ["gpl", "nonfree", "version3"])
def test_archive_rejects_contradictory_license_options(tmp_path: Path, component: str):
    source = source_tree(tmp_path)
    config = source / "ffbuild/config.mak"
    config.write_text(config.read_text().rstrip() + f" --enable-{component}\n")
    with pytest.raises(ValueError, match="licensing configuration"):
        archive.archive(source, tmp_path / "bundle.zip")


def test_search_root_associates_source_with_the_requested_artifact(tmp_path: Path):
    wheel = source_tree(tmp_path / "target/wheel/release/build/ffmpeg-sys/out")
    (wheel / "configure").write_text("wheel-specific source\n")
    wheel.rename(wheel.with_name("ffmpeg-8.0"))
    cli = source_tree(tmp_path / "target/cli/release/build/ffmpeg-sys/out")
    cli.rename(cli.with_name("ffmpeg-8.0"))
    artifact = tmp_path / "vestra.whl"
    artifact.write_bytes(b"wheel binary\n")
    output = tmp_path / "wheel-source.zip"
    subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--search-root",
            str(tmp_path / "target/wheel"),
            "--artifact",
            str(artifact),
            "--output",
            str(output),
        ],
        check=True,
    )
    with zipfile.ZipFile(output) as bundle:
        assert bundle.read("ffmpeg/configure") == b"wheel-specific source\n"
        info = json.loads(bundle.read("BUILD.json"))
        assert info["artifacts"][0]["name"] == "vestra.whl"
        assert (
            info["revision"]
            == subprocess.check_output(
                ["git", "-C", str(wheel.with_name("ffmpeg-8.0")), "rev-parse", "HEAD"],
                text=True,
            ).strip()
        )


@pytest.mark.parametrize("count", [0, 2])
def test_search_root_rejects_missing_or_ambiguous_builds(tmp_path: Path, count: int):
    for index in range(count):
        source = source_tree(tmp_path / f"target/wheel/build-{index}/out")
        source.rename(source.with_name("ffmpeg-8.0"))
    artifact = tmp_path / "vestra.whl"
    artifact.write_bytes(b"wheel binary\n")
    output = tmp_path / "source.zip"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--search-root",
            str(tmp_path / "target/wheel"),
            "--artifact",
            str(artifact),
            "--output",
            str(output),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0
    assert f"expected exactly one FFmpeg build source, found {count}" in result.stderr
    assert not output.exists()


def test_missing_artifact_does_not_create_source_archive(tmp_path: Path):
    source = source_tree(tmp_path)
    output = tmp_path / "bundle.zip"
    with pytest.raises(FileNotFoundError):
        archive.archive(source, output, artifacts=[tmp_path / "missing.whl"])
    assert not output.exists()


def test_source_archive_cannot_overwrite_associated_artifact(tmp_path: Path):
    source = source_tree(tmp_path)
    artifact = tmp_path / "vestra.whl"
    artifact.write_bytes(b"original binary\n")
    with pytest.raises(ValueError, match="must not overwrite"):
        archive.archive(source, artifact, artifacts=[artifact])
    assert artifact.read_bytes() == b"original binary\n"
