"""Render the effect parameter reference from catalog and Python metadata."""

from __future__ import annotations

import ast
import json
from typing import Any

from .python_render import NOTICE, arguments, parameters


def render_reference(effects: list[dict[str, Any]]) -> str:
    lines = [
        "# Effect parameter reference",
        "",
        f"<!-- {NOTICE} -->",
        "",
        "Use the page contents to jump to an effect. Each table lists Python defaults,",
        "accepted ranges, units and animation support. Bounds apply when a value is supplied;",
        "a dash means the catalog provides no range or unit. Coupled constraints and",
        "source restrictions are described in [effects](effects.md).",
        "",
        "Expand **Canonical JSON and planner metadata** for exact kinds and schema defaults.",
        "Python defaults are authoring conveniences, separate from JSON schema metadata.",
        "See [effects](effects.md) for parser behavior and legacy exceptions,",
        "including the required Ascii enum fields that the schema marks optional.",
        "Pass counts are catalog estimates; actual topology can depend on parameters.",
        "",
    ]
    for effect in effects:
        python = effect["python"]
        lines += [
            f"## {python['name']} (`{effect['id']}`)",
            "",
            f"Scope: `{effect['scope']}`; stage: `{effect['stage']}`; estimated passes: "
            f"{effect['estimated_pass_count']}.",
            "",
            "| Parameter | Python default | Range and units | Animation |",
            "| --- | --- | --- | --- |",
        ]
        canonical_rows = []
        authored = {arg.arg: default for arg, default in parameters(arguments(effect))}
        units = dict(effect["units"])
        for item in effect["parameters"]:
            name, kind = item["name"], item["kind"]
            default = authored[name]
            python_default = (
                "required" if default is None else f"`{ast.unparse(default)}`"
            )
            wire = "required" if item["required"] else "optional"
            if item["default"] is not None:
                wire += f"; `{json.dumps(item['default'])}`"
            minimum = (
                item.get("integer_minimum") if kind == "integer" else item["minimum"]
            )
            maximum = (
                item.get("integer_maximum") if kind == "integer" else item["maximum"]
            )
            bounds = (
                "finite"
                if kind in {"scalar_property", "plain_track", "number", "period"}
                else "—"
            )
            if minimum is not None or maximum is not None:

                def number(value: int | float) -> str:
                    return (
                        str(int(value))
                        if isinstance(value, float) and value.is_integer()
                        else str(value)
                    )

                if maximum is None:
                    bounds = ("> " if item["minimum_exclusive"] else ">= ") + number(
                        minimum
                    )
                elif minimum is None:
                    bounds = ("< " if item["maximum_exclusive"] else "<= ") + number(
                        maximum
                    )
                else:
                    bounds = (
                        ("(" if item["minimum_exclusive"] else "[")
                        + number(minimum)
                        + ", "
                        + number(maximum)
                        + (")" if item["maximum_exclusive"] else "]")
                    )
            if kind == "enum":
                bounds = ", ".join(f"`{value}`" for value in item["enum_values"])
            elif kind in {"palette", "palette_stops"}:
                bounds = "2..16 entries; see effect constraints"
            animation = {
                "scalar_property": "keyframes + signals",
                "plain_track": "keyframes",
                "point_property": "point keyframes + signals",
            }.get(kind, "static")
            cells = [
                f"`{name}`",
                python_default,
                "; ".join(value for value in (bounds, units.get(name)) if value),
                animation,
            ]
            lines.append(
                "| " + " | ".join(cell.replace("|", r"\|") for cell in cells) + " |"
            )
            canonical_rows.append(
                "    | "
                + " | ".join(
                    cell.replace("|", r"\|")
                    for cell in (f"`{name}`", f"`{kind}`", wire)
                )
                + " |"
            )
        lines += [
            "",
            '??? info "Canonical JSON and planner metadata"',
            "",
            f"    Catalog retention: `{str(effect['retains_original']).lower()}`; "
            f"temporal policy: `{effect['temporal_policy']}`.",
            "",
            "    | Parameter | Kind | JSON Schema requirement/default |",
            "    | --- | --- | --- |",
            *canonical_rows,
            "",
        ]
    return "\n".join(lines)
