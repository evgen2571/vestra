"""Frozen pre-generation contracts from origin/develop; never refresh from generated output."""

from __future__ import annotations

from copy import deepcopy
from enum import Enum
from hashlib import sha256
import inspect
from importlib import import_module
import json
from pathlib import Path
import re

from jsonschema import Draft202012Validator
import pytest

import vestra
import vestra.authoring as authoring
import vestra.authoring.effects as advanced
import vestra.effects as effects
from test_authoring_effects import _valid_generic_parameters, builder
from test_high_level_effects import _effect_values


FIXTURES = Path(__file__).parent / "fixtures" / "effects"
API = json.loads((FIXTURES / "python-api.json").read_text())
BEHAVIOR = json.loads((FIXTURES / "python-behavior.json").read_text())
JSON_V1 = json.loads((FIXTURES / "json-v1.json").read_text())
MODULES = {module.__name__: module for module in (vestra, authoring, advanced, effects)}
SCHEMA_PATH = Path(__file__).parents[1] / "schemas" / "project.schema.json"
VALIDATOR = Draft202012Validator(json.loads(SCHEMA_PATH.read_text()))


def _surface(value):
    if inspect.isfunction(value):
        return str(inspect.signature(value))
    if issubclass(value, Enum):
        return {"enum": {member.name: member.value for member in value}}
    members = {}
    for name in sorted(member for member in dir(value) if not member.startswith("_")):
        member = inspect.getattr_static(value, name)
        if isinstance(member, property):
            members[name] = {
                "get": str(inspect.signature(member.fget)),
                "set": str(inspect.signature(member.fset)) if member.fset else None,
            }
        elif callable(member):
            members[name] = str(inspect.signature(member))
    common = API["common_members"][value.__module__]
    members = {
        name: member
        for name, member in members.items()
        if (name not in common or member != common[name])
        and not (
            value.__name__.endswith("EffectCollection") and name.startswith("add_")
        )
    }
    return {
        "module": value.__module__,
        "signature": str(inspect.signature(value)),
        "members": members,
    }


@pytest.mark.parametrize("module_name", API["modules"])
def test_effect_exports_signatures_annotations_and_properties_match_baseline(
    module_name,
):
    module = MODULES[module_name]
    expected = API["modules"][module_name]
    assert list(getattr(module, "__all__", [])) == expected["exports"]
    # Also catches accidental imports/classes outside an explicit __all__.
    names = []
    for name in sorted(
        getattr(module, "__all__", [n for n in vars(module) if not n.startswith("_")])
    ):
        value = getattr(module, name)
        if (
            inspect.isclass(value)
            and (
                issubclass(value, (advanced.Effect, effects.Effect))
                or name
                in {"ActiveInterval", "ClipEffectCollection", "PostEffectCollection"}
                or issubclass(value, Enum)
                and value.__module__ == advanced.__name__
            )
            or inspect.isfunction(value)
            and name in {"available_effects", "effect_definition"}
        ):
            names.append(name)
    assert names == sorted(expected["surface"])
    for name, reference in expected["surface"].items():
        value = getattr(module, name)
        assert _surface(value) == API["surfaces"][reference], f"{module_name}.{name}"
        if reference.startswith("vestra.effects."):
            assert value is getattr(effects, name)
        elif reference.startswith("vestra.authoring.effects."):
            assert value is getattr(advanced, name)


@pytest.mark.parametrize("module_name", API["source_modules"])
def test_original_effect_source_modules_keep_public_imports(module_name):
    module = import_module(module_name)
    expected = API["source_modules"][module_name]
    assert getattr(module, "__all__", None) == expected["exports"]
    for name in expected["classes"]:
        assert getattr(module, name) is getattr(effects, name)


@pytest.mark.parametrize(
    "collection", [advanced.ClipEffectCollection, advanced.PostEffectCollection]
)
def test_effect_collection_helper_signatures_and_absences_match_baseline(collection):
    expected = dict(API["helpers"]["common"])
    if collection is advanced.ClipEffectCollection:
        expected.update(API["helpers"]["clip_only"])
    actual = {
        name: str(inspect.signature(getattr(collection, name)))
        for name in dir(collection)
        if name.startswith("add_")
    }
    assert actual == expected


def test_catalog_alias_and_all_authoring_paths_have_reconciled_coverage():
    catalog_ids = {item["id"] for item in advanced.available_effects()}
    constructors = {
        name: getattr(effects, name)
        for name in effects.__all__
        if inspect.isclass(getattr(effects, name))
        and issubclass(getattr(effects, name), effects.Effect)
        and getattr(effects, name) is not effects.Effect
    }
    assert len(catalog_ids) == 23
    assert len(constructors) == 24
    assert {
        constructor.effect_type for constructor in constructors.values()
    } == catalog_ids
    assert set(_effect_values()) == catalog_ids
    pseudo = effects.PseudoAscii().to_canonical()
    assert pseudo["type"] == "ascii"
    assert pseudo["glyph_style"] == "geometric"
    assert effects.Ascii().to_canonical()["glyph_style"] == "characters"
    typed = {
        name.removeprefix("add_")
        for name in dir(advanced.ClipEffectCollection)
        if name.startswith("add_") and name != "add_effect"
    }
    assert len(typed) == 21
    assert catalog_ids - typed == {"motion_tile", "radial_blur"}
    _, clip = builder()
    for effect_id in sorted(catalog_ids):
        handle = clip.effects.add_effect(
            effect_id,
            **_valid_generic_parameters(advanced.effect_definition(effect_id)),
        )
        assert handle.kind == effect_id
    for effect_id in catalog_ids - typed:
        assert not hasattr(advanced.ClipEffectCollection, "add_" + effect_id)
        assert not hasattr(advanced.PostEffectCollection, "add_" + effect_id)


@pytest.mark.parametrize("effect_id", BEHAVIOR["serialization"])
def test_effect_class_structure_serialization_and_keyframes_match_baseline(effect_id):
    effect = _effect_values()[effect_id]
    cls = type(effect)
    structure = {
        "mro": [base.__name__ for base in cls.__mro__],
        "slots": {
            base.__name__: list(base.__dict__["__slots__"])
            for base in cls.__mro__
            if "__slots__" in base.__dict__
        },
        "has_dict": hasattr(effect, "__dict__"),
        "public_module": cls.__module__,
    }
    assert structure == API["class_structure"][effect_id]
    expected = BEHAVIOR["serialization"][effect_id]
    assert effect.to_canonical() == expected["high_level"]
    _, clip = builder()
    generic = clip.effects.add_effect(
        effect_id, **_valid_generic_parameters(advanced.effect_definition(effect_id))
    )
    assert generic.to_canonical() == expected["generic"]
    for name in BEHAVIOR["numeric_parameters"][effect_id]:
        prop = getattr(effect, name)
        if hasattr(prop, "keyframe"):
            prop.keyframe(0.5, prop.value)
    assert effect.to_canonical() == expected["high_level_animated"]


def _attempt(call):
    try:
        value = call()
        return {
            "accepted": True,
            "canonical": value.to_canonical()
            if hasattr(value, "to_canonical")
            else value,
        }
    except Exception as error:
        return {
            "accepted": False,
            "exception": type(error).__name__,
            "message": str(error),
        }


@pytest.mark.parametrize("effect_id", BEHAVIOR["serialization"])
def test_effect_numeric_boundaries_match_frozen_values(effect_id):
    effect = _effect_values()[effect_id]
    for case, expected in BEHAVIOR["validation"][effect_id].items():
        name, bound, where = case.split(":")
        parameter = BEHAVIOR["numeric_parameters"][effect_id][name]
        prop = getattr(effect, name)
        original = prop.value if hasattr(prop, "value") else prop
        value = parameter[
            f"integer_{bound}" if parameter["kind"] == "integer" else bound
        ]
        if where == "outside":
            value += -1 if bound == "minimum" else 1

        def set_value():
            if hasattr(prop, "value"):
                prop.value = value
            else:
                setattr(effect, name, value)

        assert _attempt(set_value) == expected, f"{effect_id}:{case}"
        if hasattr(prop, "value"):
            prop.value = original
        else:
            setattr(effect, name, original)


def test_effect_validation_timing_and_errors_match_baseline():
    shake = dict(
        position_amount=0,
        rotation_degrees=0,
        scale_amount=0,
        frequency=1,
        seed=0,
        attack=-0.1,
        decay=1,
    )
    actual = {
        "typed_camera_shake_deferred": _attempt(
            lambda: builder()[1].effects.add_camera_shake(**shake)
        ),
        "generic_camera_shake_eager": _attempt(
            lambda: builder()[1].effects.add_effect("camera_shake", **shake)
        ),
        "vignette_softness_zero": _attempt(
            lambda: effects.Vignette(0.5, 1, 0, "#000000")
        ),
        "unknown_generic_effect": _attempt(
            lambda: builder()[1].effects.add_effect("unknown")
        ),
        "unknown_generic_parameter": _attempt(
            lambda: builder()[1].effects.add_effect("vignette", unused=0)
        ),
        "missing_generic_parameter": _attempt(
            lambda: builder()[1].effects.add_effect("vignette")
        ),
        "direct_advanced_construction": _attempt(lambda: advanced.VignetteEffect()),
    }
    assert actual == BEHAVIOR["validation"]["exceptions"]
    project, clip = builder()
    clip.effects.add_camera_shake(**shake)
    assert not project.validate().is_valid


def _apply_delta(base, delta):
    result = deepcopy(base)
    result.update(delta.get("set", {}))
    for name in delta.get("omit", []):
        result.pop(name)
    return result


def _parse_message(message):
    # Serde's set-like expected-field lists may be emitted in a different order.
    return re.sub(
        r"expected one of (.*)",
        lambda match: "expected one of " + ", ".join(sorted(match[1].split(", "))),
        message,
    )


@pytest.mark.parametrize(
    "case", JSON_V1["cases"], ids=lambda case: f"{case['effect_id']}:{case['case']}"
)
def test_json_v1_schema_parser_validation_and_roundtrip_match_baseline(case):
    project = deepcopy(JSON_V1["project"])
    effect_id = case["effect_id"]
    project["visual"]["clips"][0]["effects"] = [
        _apply_delta(JSON_V1["effects"][effect_id], case["input_delta"])
    ]
    assert VALIDATOR.is_valid(project) == case["schema_valid"]
    if not case["parse_valid"]:
        with pytest.raises(getattr(vestra, case["exception_type"])) as raised:
            vestra.ProjectSnapshot.from_dict(project)
        assert _parse_message(str(raised.value)) == _parse_message(case["exception"])
        return
    snapshot = vestra.ProjectSnapshot.from_dict(project)
    expected = deepcopy(JSON_V1["roundtrip_project"])
    expected["visual"]["clips"][0]["effects"] = [
        _apply_delta(JSON_V1["roundtrip_effects"][effect_id], case["roundtrip_delta"])
    ]
    assert snapshot.to_dict() == expected
    assert vestra.ProjectSnapshot.from_dict(expected).to_dict() == expected
    report = vestra.Editor().validate(snapshot)
    assert report.is_valid == case["semantic_valid"]
    assert [
        {
            "code": diagnostic.code,
            "pointer": diagnostic.pointer,
            "message": diagnostic.message,
        }
        for diagnostic in report.diagnostics
    ] == case["diagnostics"]


def test_generated_schema_is_byte_identical_to_pre_refactor_baseline():
    assert sha256(SCHEMA_PATH.read_bytes()).hexdigest() == JSON_V1["schema_sha256"]


@pytest.mark.parametrize("field", ["glyph_style", "mode", "color_mode"])
def test_ascii_omitted_enum_schema_parser_mismatch_is_preserved(field):
    project = deepcopy(JSON_V1["project"])
    effect = deepcopy(JSON_V1["effects"]["ascii"])
    effect.pop(field)
    project["visual"]["clips"][0]["effects"] = [effect]
    assert VALIDATOR.is_valid(project)
    with pytest.raises(vestra.ProjectError, match=f"missing field `{field}`"):
        vestra.ProjectSnapshot.from_dict(project)
