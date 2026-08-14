from pathlib import Path

import pytest

import vestra
from vestra import FrameRate
from vestra.authoring import DurationMode, ProjectBuilder


def builder(**changes: object) -> ProjectBuilder:
    arguments: dict[str, object] = {
        "width": 160,
        "height": 90,
        "frame_rate": FrameRate(30, 1),
        "output_path": "canonical-output.mp4",
        "duration": 1.0,
        "background": "#FF0000",
    }
    arguments.update(changes)
    return ProjectBuilder(**arguments)  # type: ignore[arg-type]


def test_background_only_builder_emits_canonical_project() -> None:
    data = builder().to_dict()
    assert data == {
        "schema_version": 2,
        "output": {
            "path": "canonical-output.mp4", "width": 160, "height": 90,
            "frame_rate": "30/1", "background": "#ff0000", "quality": "balanced",
            "audio": False, "duration_mode": "explicit", "duration": 1.0,
        },
        "assets": [],
        "visual": {"clips": [], "transitions": [], "flashes": [], "post_effects": []},
    }


def test_snapshots_and_native_projects_are_isolated(tmp_path: Path) -> None:
    authored = builder(base_directory=tmp_path, name="first", metadata={"tags": ["night"]})
    first = authored.to_dict()
    second = authored.to_dict()
    assert first == second
    assert first is not second
    first["metadata"]["tags"].append("changed")  # type: ignore[index]
    assert authored.to_dict() == second
    project = authored.build()
    before = project.to_dict()
    authored.name = "later"
    assert project.to_dict() == before
    assert project.base_directory == tmp_path
    assert isinstance(project, vestra.ProjectSnapshot)


def test_build_validate_and_native_round_trip() -> None:
    authored = builder()
    project = authored.build()
    assert authored.validate().is_valid
    reloaded = vestra.ProjectSnapshot.from_dict(project.to_dict(), base_directory=project.base_directory)
    assert reloaded.to_dict() == project.to_dict()


def test_native_validation_receives_invalid_finite_semantics() -> None:
    report = builder(width=3).validate()
    assert not report.is_valid
    assert any(item.code == "MVP-OUTPUT-WIDTH" for item in report.diagnostics)


def test_duration_modes_are_unambiguous() -> None:
    automatic = builder(duration=None)
    assert automatic.to_dict()["output"]["duration_mode"] == "automatic"  # type: ignore[index]
    with pytest.raises(ValueError, match="requires duration"):
        builder(duration=None, duration_mode=DurationMode.EXPLICIT)
    with pytest.raises(ValueError, match="must not specify duration"):
        builder(duration=1.0, duration_mode=DurationMode.AUTOMATIC)


def test_mutation_uses_constructor_validation_and_keeps_duration_coherent() -> None:
    authored = builder()
    authored.quality = vestra.authoring.Quality.HIGH
    authored.background = "#101018"
    authored.duration = None
    assert authored.to_dict()["output"].get("duration") is None  # type: ignore[index]
    assert authored.duration_mode is DurationMode.AUTOMATIC
    authored.duration = 2.0
    assert authored.duration_mode is DurationMode.EXPLICIT
    with pytest.raises(TypeError):
        authored.quality = "invalid"  # type: ignore[assignment]
    with pytest.raises(ValueError):
        authored.background = "invalid"
    with pytest.raises(TypeError):
        authored.width = True  # type: ignore[assignment]
    with pytest.raises(ValueError):
        authored.duration = float("inf")
    with pytest.raises(ValueError, match="requires duration"):
        authored.duration = None
        authored.duration_mode = DurationMode.EXPLICIT


def test_canonical_snapshot_omits_optional_nulls_except_metadata_values() -> None:
    data = builder(duration=None, metadata={"nested": [None]}).to_dict()

    def assert_no_optional_none(value: object, *, in_metadata: bool = False) -> None:
        if isinstance(value, dict):
            for key, item in value.items():
                assert_no_optional_none(item, in_metadata=in_metadata or key == "metadata")
        elif isinstance(value, list):
            for item in value:
                assert_no_optional_none(item, in_metadata=in_metadata)
        elif value is None:
            assert in_metadata

    assert_no_optional_none(data)
    assert "name" not in data
    assert "audio" not in data
    assert "metadata" in data
    assert "duration" not in data["output"]  # type: ignore[operator]
    assert all(not key.startswith("_") for key in data)
