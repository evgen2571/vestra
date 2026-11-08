"""Release inventory binds every distributed binary to exact source and hashes."""

import hashlib
import importlib.util
import json
from pathlib import Path
import zipfile

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/release-manifest.py"
REVISION = "a" * 40


def module():
    spec = importlib.util.spec_from_file_location("release_manifest", SCRIPT)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def artifacts(root):
    targets = {
        "x86_64-unknown-linux-gnu": "manylinux_2_28_x86_64",
        "aarch64-unknown-linux-gnu": "manylinux_2_28_aarch64",
        "x86_64-apple-darwin": "macosx_11_0_x86_64",
        "aarch64-apple-darwin": "macosx_11_0_arm64",
        "x86_64-pc-windows-msvc": "win_amd64",
    }
    for target, platform in targets.items():
        wheel = root / f"vestra-0.1.4-cp311-abi3-{platform}.whl"
        suffix = ".zip" if "windows" in target else ".tar.gz"
        cli = root / f"ve-0.1.4-{target}{suffix}"
        for kind, binary in [("wheel", wheel), ("cli", cli)]:
            binary.write_bytes(b"fixture")
            digest = hashlib.sha256(binary.read_bytes()).hexdigest()
            if kind == "cli":
                binary.with_name(binary.name + ".sha256").write_text(
                    f"{digest}  {binary.name}\n"
                )
            with zipfile.ZipFile(root / f"ffmpeg-{kind}-{target}.zip", "w") as archive:
                archive.writestr(
                    "BUILD.json",
                    json.dumps(
                        {
                            "vestra_revision": REVISION,
                            "artifacts": [{"name": binary.name, "sha256": digest}],
                        }
                    ),
                )
    (root / "vestra-0.1.4.tar.gz").write_bytes(b"source")


def test_manifest_records_coordinated_artifacts_and_checksums(tmp_path):
    artifacts(tmp_path)
    release = module()
    release.write_manifest(tmp_path, "0.1.4", REVISION)
    manifest = json.loads((tmp_path / "release-manifest.json").read_text())
    assert manifest["revision"] == REVISION
    assert len(manifest["artifacts"]) == 26
    assert "release-manifest.json" in (tmp_path / "SHA256SUMS").read_text()
    release.write_manifest(tmp_path, "0.1.4", REVISION)
    assert json.loads((tmp_path / "release-manifest.json").read_text()) == manifest


def test_missing_target_fails_before_publication(tmp_path):
    artifacts(tmp_path)
    next(tmp_path.glob("*win_amd64.whl")).unlink()
    with pytest.raises(ValueError, match="wheel"):
        module().write_manifest(tmp_path, "0.1.4", REVISION)


def test_mismatched_binary_or_source_revision_fails(tmp_path):
    artifacts(tmp_path)
    next(tmp_path.glob("*.whl")).write_bytes(b"different build")
    with pytest.raises(ValueError, match="source association"):
        module().write_manifest(tmp_path, "0.1.4", REVISION)


def test_retry_rejects_different_already_published_distribution(tmp_path):
    artifact = tmp_path / "vestra.whl"
    artifact.write_bytes(b"local")
    with pytest.raises(ValueError, match="already contains a different"):
        module().verify_existing(
            [artifact],
            {
                "urls": [
                    {
                        "filename": artifact.name,
                        "digests": {"sha256": "0" * 64},
                    }
                ]
            },
        )
    module().verify_existing(
        [artifact],
        {
            "urls": [
                {
                    "filename": artifact.name,
                    "digests": {"sha256": hashlib.sha256(b"local").hexdigest()},
                }
            ]
        },
    )


def test_retry_cannot_replace_source_for_already_staged_binary(tmp_path):
    current = tmp_path / "current"
    staged = tmp_path / "staged"
    current.mkdir()
    staged.mkdir()
    (current / "ffmpeg.zip").write_bytes(b"new source association")
    (staged / "ffmpeg.zip").write_bytes(b"original source association")
    with pytest.raises(ValueError, match="staged release"):
        module().verify_staged(current, staged)
    (current / "ffmpeg.zip").write_bytes(b"original source association")
    module().verify_staged(current, staged)
