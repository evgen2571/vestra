#!/usr/bin/env python3
"""Generate effect authoring declarations from the core catalog (stdlib only)."""

from __future__ import annotations

import argparse
import json
import runpy
from pathlib import Path
import subprocess
import sys
from typing import Any

from effects import python_render, reference_render

ROOT = Path(__file__).resolve().parents[1]


def with_python(
    metadata: dict[str, Any],
    overrides: dict[str, Any],
    enum_exports: dict[str, Any] | None = None,
) -> dict[str, Any]:
    ids = {effect["id"] for effect in metadata["effects"]}
    if set(overrides) - ids:
        raise ValueError(
            f"Python overrides reference unknown effects: {sorted(set(overrides) - ids)}"
        )
    effects = []
    for effect in metadata["effects"]:
        python = dict(overrides.get(effect["id"], {}))
        python["name"] = "".join(word.title() for word in effect["id"].split("_"))
        python["module"] = "vestra.effects." + python.get("module", "stylize")
        python.setdefault("base", "Effect")
        python.setdefault("doc", None)
        python["enums"] = {
            **{
                parameter["name"]: python.get("enums", {}).get(
                    parameter["name"],
                    python["name"]
                    + "".join(word.title() for word in parameter["name"].split("_")),
                )
                for parameter in effect["parameters"]
                if parameter["kind"] == "enum"
            },
            **python.get("enums", {}),
        }
        factory = python.pop(
            "helper", "amount" if python["base"] == "_AmountEffect" else "positional"
        )
        python["collection"] = (
            None
            if factory is None
            else {"factory": factory, "parameter_order": python.pop("helper_order", [])}
        )
        effects.append(
            {**effect, "python": python, "units": list(python.pop("units", {}).items())}
        )
    return {**metadata, "effects": effects, "enum_exports": enum_exports or {}}


def validate(metadata: dict[str, Any]) -> None:
    ids: set[str] = set()
    names: set[str] = set()
    enums: dict[str, list[str]] = {}
    for effect in metadata["effects"]:
        identifier = effect["id"]
        python = effect["python"]
        if not isinstance(python, dict):
            raise ValueError(f"{identifier}: missing Python authoring metadata")
        if identifier in ids or python["name"] in names:
            raise ValueError(f"{identifier}: duplicate effect id or Python name")
        ids.add(identifier)
        names.add(python["name"])
        if (
            not python["module"].startswith("vestra.effects.")
            or not python["module"].removeprefix("vestra.effects.").isidentifier()
            or not python["name"].isidentifier()
        ):
            raise ValueError(f"{identifier}: invalid Python module or class name")
        if python.get("advanced_serialization", "registered") not in {
            "registered",
            "direct",
        }:
            raise ValueError(f"{identifier}: invalid advanced serialization policy")
        expected = [item["name"] for item in effect["parameters"]]
        if len(expected) != len(set(expected)):
            raise ValueError(f"{identifier}: duplicate parameter name")
        for key in ("enums", "annotations", "defaults", "expressions"):
            if set(python.get(key, {})) - set(expected):
                raise ValueError(f"{identifier}: {key} reference an unknown parameter")
        if set(python.get("defaults", {})) & set(python.get("expressions", {})):
            raise ValueError(f"{identifier}: duplicate default override")
        args = python_render.arguments(effect)
        authored = python_render.parameters(args)
        actual = [arg.arg for arg, _ in authored if arg.arg not in {"self", "id"}]
        if any(arg.annotation is None for arg, _ in authored if arg.arg != "self"):
            raise ValueError(
                f"{identifier}: every public argument needs a type annotation"
            )
        for item in effect["parameters"]:
            if item["kind"] not in python_render.KINDS:
                raise ValueError(
                    f"{identifier}.{item['name']}: unsupported kind {item['kind']!r}"
                )
            for prefix in ("", "integer_"):
                minimum, maximum = (
                    item.get(prefix + "minimum"),
                    item.get(prefix + "maximum"),
                )
                if minimum is not None and maximum is not None and minimum > maximum:
                    raise ValueError(f"{identifier}.{item['name']}: inverted bounds")
            if item["kind"] == "enum" and (
                not item["enum_values"] or item["default"] not in item["enum_values"]
            ):
                raise ValueError(f"{identifier}.{item['name']}: invalid enum/default")
            if item["kind"] == "enum":
                name = python["enums"][item["name"]]
                members = [value.upper() for value in item["enum_values"]]
                if len(members) != len(set(members)):
                    raise ValueError(f"Python enum {name} contains duplicate members")
                if any(not member.isidentifier() for member in members):
                    raise ValueError(
                        f"Python enum {name} contains an invalid member name"
                    )
                if name in enums and enums[name] != item["enum_values"]:
                    raise ValueError(
                        f"Python enum {name} has inconsistent ordered values"
                    )
                enums[name] = item["enum_values"]
        if set(dict(effect["units"])) - set(expected):
            raise ValueError(f"{identifier}: units reference an unknown parameter")
        collection = python["collection"]
        if collection is not None:
            if collection["factory"] not in {
                "positional",
                "amount",
                "palette",
                "stylized",
            }:
                raise ValueError(f"{identifier}: unsupported collection factory")
            order = collection["parameter_order"] or actual
            if len(order) != len(actual) or set(order) != set(actual):
                raise ValueError(
                    f"{identifier}: helper order must cover every parameter once"
                )
    for effect in metadata["effects"]:
        base = effect["python"]["base"]
        if base not in names | {"Effect", "_AmountEffect"}:
            raise ValueError(f"{effect['id']}: unsupported base {base!r}")
    if set(metadata.get("enum_exports", {})) - set(enums):
        raise ValueError("Python enum export layout references an unknown enum")
    if not isinstance(metadata["schema"], str) or not metadata["schema"].endswith("\n"):
        raise ValueError("core export must include the canonical schema text")
    schema = json.loads(metadata["schema"])
    branches = {
        item["$ref"].removeprefix("#/$defs/").removesuffix("_effect")
        for item in schema["$defs"]["effect"]["oneOf"]
    }
    if branches != ids:
        raise ValueError("schema effect branches do not match the catalog")


def outputs(metadata: dict[str, Any], root: Path) -> dict[Path, str]:
    validate(metadata)
    return {
        **python_render.outputs(metadata, root),
        Path("schemas/project.schema.json"): metadata["schema"],
        Path("docs/reference/effect-parameters.md"): reference_render.render_reference(
            metadata["effects"]
        ),
    }


def synchronize(expected: dict[Path, str], root: Path, *, check: bool) -> list[Path]:
    stale = [
        path
        for path, content in expected.items()
        if not (root / path).exists() or (root / path).read_bytes() != content.encode()
    ]
    if not check:
        for path in stale:
            (root / path).parent.mkdir(parents=True, exist_ok=True)
            (root / path).write_bytes(expected[path].encode())
    return stale


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check", action="store_true", help="report stale artifacts without writing"
    )
    options = parser.parse_args()
    try:
        completed = subprocess.run(
            [
                "cargo",
                "run",
                "--locked",
                "--quiet",
                "-p",
                "vestra-core",
                "--example",
                "effect_metadata",
                "--",
                "schemas/project.schema.json",
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        python_metadata = runpy.run_path(
            str(ROOT / "scripts/effects/python_metadata.py")
        )
        metadata = with_python(
            json.loads(completed.stdout),
            python_metadata["PYTHON"],
            python_metadata["ENUM_EXPORTS"],
        )
        stale = synchronize(outputs(metadata, ROOT), ROOT, check=options.check)
    except subprocess.CalledProcessError as error:
        print(error.stderr, file=sys.stderr)
        return 1
    except (ValueError, KeyError, OSError, SyntaxError) as error:
        print(f"effect generation failed: {error}", file=sys.stderr)
        return 1
    if stale and options.check:
        print(
            "Stale effect artifacts; run just gen-effects:\n"
            + "\n".join(map(str, stale)),
            file=sys.stderr,
        )
        return 1
    print(
        f"Effect artifacts {'checked' if options.check else 'updated'} ({len(stale)} changed)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
