"""Corresponding-source archives keep edits and omit machine paths/build junk."""

import importlib.util
import json
from pathlib import Path
import subprocess
import zipfile

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/archive-ffmpeg-source.py"
spec = importlib.util.spec_from_file_location("ffmpeg_archive", SCRIPT)
archive = importlib.util.module_from_spec(spec)
spec.loader.exec_module(archive)


def source_tree(tmp_path: Path) -> Path:
    source = tmp_path / "source"
    source.mkdir()
    subprocess.run(["git", "init", "-q", str(source)], check=True)
    (source / "configure").write_text("original source\n")
    subprocess.run(["git", "-C", str(source), "add", "configure"], check=True)
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
        "FFMPEG_CONFIGURATION=--prefix=/private/build --extra-cflags='-I/private/includes -w' "
        "--disable-gpl --disable-nonfree --disable-version3 --enable-static\n"
    )
    (source / "generated.o").write_bytes(b"private object data")
    return source


def test_archive_contains_corresponding_source_and_patch_without_machine_metadata(
    tmp_path: Path,
):
    source = source_tree(tmp_path)
    output = tmp_path / "bundle.zip"
    archive.archive(source, output)
    with zipfile.ZipFile(output) as bundle:
        assert bundle.read("ffmpeg/configure") == b"modified source\n"
        assert b"+modified source" in bundle.read("CHANGES.diff")
        info = json.loads(bundle.read("BUILD.json"))
        assert len(info["revision"]) == 40
        assert "--enable-static" in info["configure_options"]
        assert b"/private/" not in bundle.read("BUILD.json")
        assert "ffmpeg/generated.o" not in bundle.namelist()
        assert not any(".git/" in name for name in bundle.namelist())
        assert "vestra-build.rs" in bundle.namelist()


def test_archive_rejects_unexpected_license_configuration(tmp_path: Path):
    source = source_tree(tmp_path)
    config = source / "ffbuild/config.mak"
    config.write_text(config.read_text().replace("--disable-gpl", "--enable-gpl"))
    with pytest.raises(ValueError, match="licensing configuration"):
        archive.archive(source, tmp_path / "bundle.zip")
