#!/usr/bin/env python3
"""Check release contracts and freeze the source identity before artifact builds."""

import argparse
from pathlib import Path
import subprocess
import tomllib


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def metadata(root: Path, ref: str, *, publish: bool) -> dict[str, str]:
    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
    version = workspace["package"]["version"]
    package = tomllib.loads((root / "pyproject.toml").read_text())["project"]
    if package["dynamic"] != ["version"] or package["license"] != "MIT":
        raise ValueError("Python version/license contract changed")
    for member in workspace["members"]:
        crate = tomllib.loads((root / member / "Cargo.toml").read_text())["package"]
        if crate["version"] != {"workspace": True} or crate["license"] != "MIT":
            raise ValueError(f"{member} must inherit the workspace version and use MIT")
    revision = git(root, "rev-parse", "HEAD")
    tag = ref.removeprefix("refs/tags/")
    if publish and tag != f"v{version}":
        raise ValueError("publication requires the exact workspace version tag")
    if tag == f"v{version}":
        if git(root, "rev-parse", f"refs/tags/{tag}^{{commit}}") != revision:
            raise ValueError("version tag does not identify the checked-out revision")
    else:
        tag = ""
    return {"version": version, "revision": revision, "tag": tag}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ref", required=True)
    parser.add_argument("--publish", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = metadata(
        Path(__file__).resolve().parents[1], args.ref, publish=args.publish
    )
    lines = "".join(f"{key}={value}\n" for key, value in result.items())
    print(lines, end="")
    if args.output:
        with args.output.open("a") as stream:
            stream.write(lines)


if __name__ == "__main__":
    main()
