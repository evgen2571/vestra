import json
from pathlib import Path

import vestra


PROJECT = {
    "schema_version": 1,
    "output": {
        "path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1",
        "background": "#000000", "quality": "balanced", "audio": False,
        "duration_mode": "automatic",
    },
    "assets": [],
    "visual": {"clips": []},
}


def test_project_dictionary_round_trip(tmp_path: Path) -> None:
    project = vestra.ProjectSnapshot.from_dict(PROJECT, base_directory=tmp_path)
    assert vestra.ProjectSnapshot.from_json(project.to_json()).to_dict() == project.to_dict()
    output = tmp_path / "project.json"
    project.save(output)
    loaded = vestra.ProjectSnapshot.load(output)
    assert loaded.source_path == output
    assert loaded.base_directory == tmp_path


def test_invalid_mapping_key_is_rejected() -> None:
    try:
        vestra.ProjectSnapshot.from_dict({1: "not valid"})  # type: ignore[dict-item]
    except TypeError:
        pass
    else:
        raise AssertionError("non-string key was accepted")


class CustomPath:
    def __init__(self, path: Path) -> None:
        self.path = path

    def __fspath__(self) -> str:
        return str(self.path)


def test_project_errors_and_custom_pathlike(tmp_path: Path) -> None:
    source = tmp_path / "project.json"
    payload = dict(PROJECT)
    payload["schema_version"] = 2
    source.write_text(json.dumps(payload), encoding="utf-8")
    try:
        vestra.ProjectSnapshot.load(CustomPath(source))
    except vestra.ProjectError as error:
        assert error.kind == "project"
        assert isinstance(error.diagnostics, tuple)
        assert isinstance(error.warnings, tuple)
        assert error.diagnostics[0].code == "VESTRA-SCHEMA-VERSION"
    else:
        raise AssertionError("unsupported schema was accepted")

    try:
        vestra.ProjectSnapshot.from_json("not json")
    except vestra.ProjectError as error:
        assert error.kind == "project"
        assert isinstance(error.diagnostics, tuple)
        assert isinstance(error.warnings, tuple)
        assert error.diagnostics[0].code == "VESTRA-PROJECT-SHAPE"
    else:
        raise AssertionError("invalid JSON was accepted")
