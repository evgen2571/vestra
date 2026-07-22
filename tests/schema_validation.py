#!/usr/bin/env python3
"""Validate the canonical project schema against representative inputs."""

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
schema = json.loads((ROOT / "schemas/project.schema.json").read_text())
project = json.loads((ROOT / "examples/projects/animation-effects.json").read_text())
validator = Draft202012Validator(schema)


def errors(instance):
    return list(validator.iter_errors(instance))


assert not errors(project), "canonical example must validate"

solid_colour = copy.deepcopy(project)
solid_colour["assets"] = []
solid_colour["visual"]["clips"] = [
    {
        "id": "canvas-colour",
        "source": {"type": "solid_color", "colour": "#112233"},
        "start": 0,
        "duration": 0.1,
        "layer": 0,
        "opacity": {"base_value": 1},
    }
]
solid_colour["visual"]["transitions"] = []
assert not errors(solid_colour), "solid colours do not need transforms"

solid_with_transform = copy.deepcopy(solid_colour)
solid_with_transform["visual"]["clips"][0]["transform"] = project["visual"]["clips"][0]["transform"]
assert errors(solid_with_transform), "solid colours must not accept transforms"

wrong_position = copy.deepcopy(project)
wrong_position["visual"]["clips"][0]["transform"]["position"]["base_value"] = 1
assert errors(wrong_position), "scalar position must not validate"

wrong_opacity = copy.deepcopy(project)
wrong_opacity["visual"]["clips"][0]["opacity"]["base_value"] = 2
assert errors(wrong_opacity), "out-of-range opacity must not validate"

zero_scale = copy.deepcopy(project)
zero_scale["visual"]["clips"][0]["transform"]["scale"]["base_value"]["x"] = 0
assert errors(zero_scale), "zero scale must not validate"

invalid_anchor = copy.deepcopy(project)
invalid_anchor["visual"]["clips"][0]["transform"]["anchor"]["base_value"]["x"] = -0.1
assert errors(invalid_anchor), "out-of-range anchor must not validate"

invalid_tint = copy.deepcopy(project)
invalid_tint["visual"]["clips"][0]["effects"] = [
    {"id": "tint", "type": "tint", "colour": "#abcdef", "amount": {"base_value": 2}}
]
assert errors(invalid_tint), "out-of-range tint amount must not validate"

unknown_effect_field = copy.deepcopy(project)
unknown_effect_field["visual"]["clips"][0]["effects"][0]["unknown"] = True
assert errors(unknown_effect_field), "unknown effect fields must not validate"

unknown_transition_field = copy.deepcopy(project)
unknown_transition_field["visual"]["transitions"][0]["unknown"] = True
assert errors(unknown_transition_field), "unknown transition fields must not validate"

versioned = copy.deepcopy(project)
versioned["version"] = 2
assert errors(versioned), "project version fields must not validate"
