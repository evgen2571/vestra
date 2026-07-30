from math import inf, nan
from pathlib import Path
import types
from collections import UserDict
from collections.abc import Iterator, Mapping

import pytest

import video_editor


VALID = {
    "schema_version": 1,
    "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": False, "duration_mode": "automatic"},
    "assets": [],
    "visual": {"clips": []},
}


def test_enums_are_immutable_hashable_and_use_sdk_strings() -> None:
    assert video_editor.BackendPreference.AUTO.value == "auto"
    assert str(video_editor.Category.PROJECT) == "project"
    assert hash(video_editor.Severity.FATAL)
    assert video_editor.AdapterDeviceType.DISCRETE_GPU.value == "discretegpu"
    assert video_editor.AdapterDeviceType.INTEGRATED_GPU.value == "integratedgpu"
    assert video_editor.AdapterDeviceType.CPU.value == "cpu"
    assert video_editor.GraphicsBackend.VULKAN.value == "vulkan"
    assert video_editor.GraphicsBackend.DX12.value == "dx12"
    assert video_editor.GraphicsBackend.BROWSER_WEBGPU.value == "browserwebgpu"
    assert str(video_editor.AdapterDeviceType.DISCRETE_GPU) == "discretegpu"
    assert hash(video_editor.GraphicsBackend.VULKAN)
    assert "GraphicsBackend" in repr(video_editor.GraphicsBackend.VULKAN)
    with pytest.raises(TypeError):
        video_editor.BackendPreference()


def test_native_runtime_names_are_exact_public_names() -> None:
    import video_editor._native as native

    classes = {
        "Project": video_editor.Project,
        "Editor": video_editor.Editor,
        "PreflightOptions": video_editor.PreflightOptions,
        "Diagnostic": video_editor.Diagnostic,
        "ValidationReport": video_editor.ValidationReport,
        "PreflightReport": video_editor.PreflightReport,
        "InspectionReport": video_editor.InspectionReport,
        "InspectOutput": video_editor.InspectOutput,
        "InspectAssets": video_editor.InspectAssets,
        "InspectAudio": video_editor.InspectAudio,
        "BackendPreference": video_editor.BackendPreference,
        "Category": video_editor.Category,
        "Severity": video_editor.Severity,
        "VideoEditorError": video_editor.VideoEditorError,
        "ProjectError": video_editor.ProjectError,
        "PreparationError": video_editor.PreparationError,
        "FrameRenderError": video_editor.FrameRenderError,
        "PreparedProjectBusyError": video_editor.PreparedProjectBusyError,
        "PrepareOptions": video_editor.PrepareOptions,
        "PreparedProject": video_editor.PreparedProject,
        "PreparationReport": video_editor.PreparationReport,
        "PreparationTimings": video_editor.PreparationTimings,
        "FrameRate": video_editor.FrameRate,
        "Frame": video_editor.Frame,
        "BackendFallback": video_editor.BackendFallback,
        "AdapterInfo": video_editor.AdapterInfo,
        "AdapterDeviceType": video_editor.AdapterDeviceType,
        "GraphicsBackend": video_editor.GraphicsBackend,
    }
    for expected_name, value in classes.items():
        assert value.__name__ == expected_name
        assert value.__module__ == "video_editor._native"
    assert not any(name.startswith("Py") for name in dir(native))
    assert not any(name.startswith("_test_") for name in native.__all__)


def test_frozen_values_and_diagnostic_ownership() -> None:
    report = video_editor.Editor().validate(video_editor.Project.from_dict(VALID))
    assert isinstance(report.diagnostics, tuple)
    with pytest.raises(AttributeError):
        report.is_valid = False  # type: ignore[misc]
    with pytest.raises(AttributeError):
        video_editor.PreflightOptions.for_validation().kind = "render"  # type: ignore[misc]


@pytest.mark.parametrize("value", [nan, inf, -inf])
def test_non_finite_dict_values_are_rejected(value: float) -> None:
    payload = dict(VALID)
    payload["metadata"] = {"value": value}
    with pytest.raises(ValueError):
        video_editor.Project.from_dict(payload)


def test_project_error_has_complete_base_contract(tmp_path: Path) -> None:
    with pytest.raises(video_editor.ProjectError) as raised:
        video_editor.Project.load(tmp_path / "missing.json")
    error = raised.value
    assert error.kind == "project"
    assert isinstance(error.diagnostics, tuple)
    assert isinstance(error.warnings, tuple)
    assert error.warnings == ()
    assert error.diagnostics[0].code == "MVP-PROJECT-READ"


def test_preflight_options_keep_every_exposed_argument(tmp_path: Path) -> None:
    options = video_editor.PreflightOptions.for_render(
        video_editor.BackendPreference.CPU, tmp_path / "out.mp4", overwrite=True
    )
    assert options.kind == "render"
    assert options.backend == video_editor.BackendPreference.CPU
    assert options.output == tmp_path / "out.mp4"
    assert options.overwrite


def test_every_preflight_constructor_preserves_its_exposed_state(tmp_path: Path) -> None:
    validation = video_editor.PreflightOptions.for_validation()
    inspection = video_editor.PreflightOptions.for_inspection()
    preparation = video_editor.PreflightOptions.for_preparation(video_editor.BackendPreference.CPU)
    rendering = video_editor.PreflightOptions.for_render(
        video_editor.BackendPreference.CPU, tmp_path / "out.mp4", overwrite=True
    )
    assert (validation.kind, validation.backend, validation.output, validation.overwrite) == ("validation", None, None, False)
    assert (inspection.kind, inspection.backend, inspection.output, inspection.overwrite) == ("inspection", None, None, False)
    assert (preparation.kind, preparation.backend, preparation.output, preparation.overwrite) == ("preparation", video_editor.BackendPreference.CPU, None, False)
    assert (rendering.kind, rendering.backend, rendering.output, rendering.overwrite) == ("render", video_editor.BackendPreference.CPU, tmp_path / "out.mp4", True)


def test_from_dict_accepts_general_mappings() -> None:
    assert video_editor.Project.from_dict(types.MappingProxyType(VALID)).to_dict()["schema_version"] == 1
    assert video_editor.Project.from_dict(UserDict(VALID)).to_dict()["schema_version"] == 1

    class DeterministicMapping(Mapping[str, object]):
        def __init__(self, values: dict[str, object]) -> None:
            self.items_by_key = values

        def __getitem__(self, key: str) -> object:
            return self.items_by_key[key]

        def __iter__(self) -> Iterator[str]:
            return iter(self.items_by_key)

        def __len__(self) -> int:
            return len(self.items_by_key)

    source = DeterministicMapping({**VALID, "metadata": DeterministicMapping({"nested": True})})
    project = video_editor.Project.from_dict(source)
    del source
    assert project.to_dict()["metadata"] == {"nested": True}
