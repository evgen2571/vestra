"""Release identity must fail closed before expensive builds or publication."""

import importlib.util
from pathlib import Path
import subprocess

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/release-metadata.py"


def load_script():
    spec = importlib.util.spec_from_file_location("release_metadata", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def checkout(tmp_path):
    (tmp_path / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["crate"]\n[workspace.package]\nversion = "0.1.4"\n'
    )
    (tmp_path / "pyproject.toml").write_text(
        '[project]\ndynamic = ["version"]\nlicense = "MIT"\n'
    )
    (tmp_path / "crate").mkdir()
    (tmp_path / "crate/Cargo.toml").write_text(
        '[package]\nversion.workspace = true\nlicense = "MIT"\n'
    )
    subprocess.run(["git", "init", "-q", str(tmp_path)], check=True)
    subprocess.run(["git", "-C", str(tmp_path), "add", "."], check=True)
    subprocess.run(
        [
            "git",
            "-C",
            str(tmp_path),
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
    return tmp_path


def test_candidate_identity_is_exact_commit_and_workspace_version(tmp_path):
    root = checkout(tmp_path)
    result = load_script().metadata(root, "develop", publish=False)
    assert result["version"] == "0.1.4"
    assert (
        result["revision"]
        == subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
        ).strip()
    )
    assert result["tag"] == ""


@pytest.mark.parametrize("ref", ["develop", "v0.1.3", "refs/tags/v0.1.3"])
def test_publication_rejects_branch_or_mismatched_version(tmp_path, ref):
    with pytest.raises(ValueError, match="version tag"):
        load_script().metadata(checkout(tmp_path), ref, publish=True)


def test_tag_must_resolve_to_checked_out_revision(tmp_path):
    root = checkout(tmp_path)
    subprocess.run(
        ["git", "-C", str(root), "-c", "tag.gpgSign=false", "tag", "v0.1.4"], check=True
    )
    assert (
        load_script().metadata(root, "refs/tags/v0.1.4", publish=True)["tag"]
        == "v0.1.4"
    )
    (root / "new").touch()
    subprocess.run(["git", "-C", str(root), "add", "."], check=True)
    subprocess.run(
        [
            "git",
            "-C",
            str(root),
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "next",
        ],
        check=True,
    )
    with pytest.raises(ValueError, match="revision"):
        load_script().metadata(root, "v0.1.4", publish=True)


def test_license_and_version_inheritance_are_still_required(tmp_path):
    root = checkout(tmp_path)
    (root / "crate/Cargo.toml").write_text(
        '[package]\nversion = "0.1.4"\nlicense = "MIT"\n'
    )
    with pytest.raises(ValueError, match="workspace version"):
        load_script().metadata(root, "develop", publish=False)


def test_publication_requires_workflow_from_same_tag():
    environment = {
        "protection_rules": [
            {"type": "required_reviewers", "reviewers": [{"type": "User"}]}
        ]
    }
    script = load_script()
    script.publication_policy("v0.1.4", "refs/tags/v0.1.4", environment)
    for ref in ("refs/heads/develop", "refs/tags/v0.1.3", ""):
        with pytest.raises(ValueError, match="workflow.*version tag"):
            script.publication_policy("v0.1.4", ref, environment)


@pytest.mark.parametrize(
    "rules",
    [
        [],
        [{"type": "branch_policy"}],
        [{"type": "required_reviewers", "reviewers": []}],
    ],
)
def test_publication_rejects_environment_without_required_reviewers(rules):
    with pytest.raises(ValueError, match="pypi.*required reviewers"):
        load_script().publication_policy(
            "v0.1.4", "refs/tags/v0.1.4", {"protection_rules": rules}
        )
