"""Corresponding-source archives keep edits and omit machine paths/build junk."""

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import textwrap
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
    if count:
        paths = [
            f"build-{index}/out/ffmpeg-8.0/ffbuild/config.mak" for index in range(count)
        ]
        assert all(path in result.stderr for path in paths)
        assert result.stderr.index(paths[0]) < result.stderr.index(paths[1])
    assert not output.exists()


@pytest.mark.skipif(shutil.which("cargo") is None, reason="requires Cargo")
@pytest.mark.parametrize("kind", ["cli", "wheel"])
def test_release_cleanup_removes_stale_ffmpeg_feature_builds(tmp_path: Path, kind: str):
    fixture = source_tree(tmp_path / "fixture")
    (tmp_path / "Cargo.toml").write_text(
        '[package]\nname="ffmpeg-sys-next"\nversion="8.1.0"\nedition="2021"\n'
        "[features]\nresampling=[]\n"
    )
    (tmp_path / "src").mkdir()
    (tmp_path / "src/lib.rs").touch()
    (tmp_path / "build.rs").write_text(
        """fn main() {
    let fixture = std::env::var("FFMPEG_ARCHIVE_FIXTURE").unwrap();
    let source = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap())
        .join("ffmpeg-8.1");
    assert!(std::process::Command::new("git")
        .args(["clone", "--quiet", &fixture]).arg(&source)
        .status().unwrap().success());
    std::fs::create_dir(source.join("ffbuild")).unwrap();
    std::fs::copy(std::path::Path::new(&fixture).join("ffbuild/config.mak"),
        source.join("ffbuild/config.mak")).unwrap();
    let contents = if std::env::var_os("CARGO_FEATURE_RESAMPLING").is_some() {
        "resampling source\\n"
    } else { "cached source\\n" };
    std::fs::write(source.join("configure"), contents).unwrap();
}
"""
    )
    target = next(
        line.removeprefix("host: ")
        for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines()
        if line.startswith("host: ")
    )
    environment = dict(os.environ, TARGET=target, FFMPEG_ARCHIVE_FIXTURE=str(fixture))
    build = [
        "cargo",
        "build",
        "--release",
        "--target",
        target,
        "--target-dir",
        f"target/{kind}",
    ]
    subprocess.run(build, cwd=tmp_path, env=environment, check=True)
    subprocess.run(
        build + ["--features", "resampling"], cwd=tmp_path, env=environment, check=True
    )
    assert len(list((tmp_path / f"target/{kind}").rglob("config.mak"))) == 2
    unrelated = tmp_path / f"target/{kind}/{target}/release/deps/libunrelated.rlib"
    unrelated.touch()
    unused = tmp_path / ("target/wheel" if kind == "cli" else "target/cli")
    unused.mkdir(parents=True)
    (unused / "restored-cache").touch()
    # Cargo 1.99 rejects restored targets without its cache marker.
    cargo = shutil.which("cargo")
    tools = tmp_path / "tools"
    tools.mkdir()
    wrapper = tools / "cargo"
    wrapper.write_text(
        f"""#!{sys.executable}
import os
from pathlib import Path
import sys
target = Path(sys.argv[sys.argv.index("--target-dir") + 1])
marker = target / "CACHEDIR.TAG"
if not marker.is_file() or not marker.read_bytes().startswith(
    b"Signature: 8a477f597d28d172789f06886806bc55"
):
    sys.exit(f"error: cannot clean {{target}}: missing or invalid `CACHEDIR.TAG` file")
os.execv({cargo!r}, [{cargo!r}, *sys.argv[1:]])
"""
    )
    wrapper.chmod(0o755)
    cleanup_environment = dict(
        environment, PATH=str(tools) + os.pathsep + os.environ["PATH"]
    )

    workflow = (SCRIPT.parents[1] / ".github/workflows/artifacts.yml").read_text()
    step = workflow.partition("      - name: Clear outdated bundled FFmpeg builds\n")[2]
    step = step.split("\n      - ", 1)[0]
    cleanup = textwrap.dedent(step.partition("        run: |\n")[2])
    subprocess.run(
        ["bash", "-euo", "pipefail", "-c", cleanup],
        cwd=tmp_path,
        env=cleanup_environment,
        check=True,
    )
    assert unrelated.exists()
    assert (unused / "restored-cache").exists()
    subprocess.run(
        build + ["--features", "resampling"], cwd=tmp_path, env=environment, check=True
    )

    artifact = tmp_path / "vestra.whl"
    artifact.write_bytes(b"resampling binary\n")
    output = tmp_path / "source.zip"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--search-root",
            str(tmp_path / f"target/{kind}"),
            "--artifact",
            str(artifact),
            "--output",
            str(output),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    with zipfile.ZipFile(output) as bundle:
        assert bundle.read("ffmpeg/configure") == b"resampling source\n"


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
