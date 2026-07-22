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

wrong_position = copy.deepcopy(project)
wrong_position["visual"]["clips"][0]["transform"]["position"]["base_value"] = 1
assert errors(wrong_position), "scalar position must not validate"

unknown_effect_field = copy.deepcopy(project)
unknown_effect_field["visual"]["clips"][0]["effects"][0]["unknown"] = True
assert errors(unknown_effect_field), "unknown effect fields must not validate"

unknown_transition_field = copy.deepcopy(project)
unknown_transition_field["visual"]["transitions"][0]["unknown"] = True
assert errors(unknown_transition_field), "unknown transition fields must not validate"

versioned = copy.deepcopy(project)
versioned["version"] = 2
assert errors(versioned), "project version fields must not validate"
