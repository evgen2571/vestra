import pytest

import vestra


BASE = {
    "schema_version": 2,
    "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": False, "duration_mode": "automatic"},
    "assets": [], "visual": {"clips": []},
}


def test_json_representable_metadata_round_trips_without_stringification() -> None:
    value = dict(BASE)
    value["metadata"] = {"none": None, "bool": True, "integer": -9, "float": 1.25, "items": ("x", 2)}
    result = vestra.Project.from_dict(value).to_dict()["metadata"]
    assert result == {"none": None, "bool": True, "integer": -9, "float": 1.25, "items": ["x", 2]}


def test_huge_integer_and_custom_object_are_rejected() -> None:
    class Value:
        def __str__(self) -> str:
            return "not-json"

    huge = dict(BASE)
    huge["metadata"] = {"value": 2**100}
    with pytest.raises(ValueError):
        vestra.Project.from_dict(huge)

    custom = dict(BASE)
    custom["metadata"] = {"value": Value()}
    with pytest.raises(TypeError):
        vestra.Project.from_dict(custom)


@pytest.mark.parametrize("value", [[], "project", 123, None])
def test_from_dict_rejects_non_mappings_immediately(value: object) -> None:
    with pytest.raises(TypeError):
        vestra.Project.from_dict(value)  # type: ignore[arg-type]


def test_recursive_and_nested_non_string_mapping_values_are_rejected() -> None:
    recursive: dict[str, object] = dict(BASE)
    recursive["metadata"] = recursive
    with pytest.raises(ValueError, match="recursive"):
        vestra.Project.from_dict(recursive)
    invalid = dict(BASE)
    invalid["metadata"] = {1: "not-json"}
    with pytest.raises(TypeError, match="keys must be strings"):
        vestra.Project.from_dict(invalid)


def test_valid_mapping_with_invalid_project_schema_raises_project_error() -> None:
    invalid_schema = dict(BASE)
    invalid_schema["schema_version"] = 3
    with pytest.raises(vestra.ProjectError) as raised:
        vestra.Project.from_dict(invalid_schema)
    assert raised.value.kind == "project"
