from math import inf, nan
from pathlib import Path
import types
from collections import UserDict
from collections.abc import Iterator, Mapping

import pytest

import vestra


VALID = {
    "schema_version": 3,
    "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": False, "duration_mode": "automatic"},
    "assets": [],
    "visual": {"clips": []},
}


def test_enums_are_immutable_hashable_and_use_sdk_strings() -> None:
    assert vestra.BackendPreference.AUTO.value == "auto"
    assert str(vestra.Category.PROJECT) == "project"
    assert hash(vestra.Severity.FATAL)
    assert vestra.AdapterDeviceType.DISCRETE_GPU.value == "discretegpu"
    assert vestra.AdapterDeviceType.INTEGRATED_GPU.value == "integratedgpu"
    assert vestra.AdapterDeviceType.CPU.value == "cpu"
    assert vestra.GraphicsBackend.VULKAN.value == "vulkan"
    assert vestra.GraphicsBackend.DX12.value == "dx12"
    assert vestra.GraphicsBackend.BROWSER_WEBGPU.value == "browserwebgpu"
    assert str(vestra.AdapterDeviceType.DISCRETE_GPU) == "discretegpu"
    assert hash(vestra.GraphicsBackend.VULKAN)
    assert "GraphicsBackend" in repr(vestra.GraphicsBackend.VULKAN)
    with pytest.raises(TypeError):
        vestra.BackendPreference()


def test_native_runtime_names_are_exact_public_names() -> None:
    import vestra._native as native

    classes = {
        "Project": vestra.ProjectSnapshot,
        "Editor": vestra.Editor,
        "PreflightOptions": vestra.PreflightOptions,
        "Diagnostic": vestra.Diagnostic,
        "ValidationReport": vestra.ValidationReport,
        "PreflightReport": vestra.PreflightReport,
        "InspectionReport": vestra.InspectionReport,
        "InspectOutput": vestra.InspectOutput,
        "InspectAssets": vestra.InspectAssets,
        "InspectAudio": vestra.InspectAudio,
        "InspectAudioTrack": vestra.InspectAudioTrack,
        "InspectAudioClip": vestra.InspectAudioClip,
        "InspectAudioGainKeyframe": vestra.InspectAudioGainKeyframe,
        "BackendPreference": vestra.BackendPreference,
        "Category": vestra.Category,
        "Severity": vestra.Severity,
        "VideoEditorError": vestra.VideoEditorError,
        "ProjectError": vestra.ProjectError,
        "PreparationError": vestra.PreparationError,
        "FrameRenderError": vestra.FrameRenderError,
        "RenderError": vestra.RenderError,
        "CancelledError": vestra.CancelledError,
        "PreparedProjectBusyError": vestra.PreparedProjectBusyError,
        "PrepareOptions": vestra.PrepareOptions,
        "PreparedProject": vestra.PreparedProject,
        "PreparationReport": vestra.PreparationReport,
        "PreparationTimings": vestra.PreparationTimings,
        "FrameRate": vestra.FrameRate,
        "Frame": vestra.Frame,
        "BackendFallback": vestra.BackendFallback,
        "AdapterInfo": vestra.AdapterInfo,
        "AdapterDeviceType": vestra.AdapterDeviceType,
        "GraphicsBackend": vestra.GraphicsBackend,
        "RenderRequest": vestra.RenderRequest,
        "PreparedVideoRenderRequest": vestra.PreparedVideoRenderRequest,
        "CancellationToken": vestra.CancellationToken,
        "RenderEvent": vestra.RenderEvent,
        "RenderResult": vestra.RenderResult,
        "RenderTimingScope": vestra.RenderTimingScope,
        "RenderTimings": vestra.RenderTimings,
        "RenderPerformance": vestra.RenderPerformance,
        "RenderFailureContext": vestra.RenderFailureContext,
        "RenderFailureStage": vestra.RenderFailureStage,
    }
    for expected_name, value in classes.items():
        assert value.__name__ == expected_name
        assert value.__module__ == "vestra._native"
    assert not any(name.startswith("Py") for name in dir(native))
    assert not any(name.startswith("_test_") for name in native.__all__)


def test_audio_inspection_dtos_are_top_level_public_symbols() -> None:
    names = {
        "InspectAudio",
        "InspectAudioTrack",
        "InspectAudioClip",
        "InspectAudioGainKeyframe",
    }
    assert names <= set(vestra.__all__)
    assert all(hasattr(vestra, name) for name in names)


def test_frozen_values_and_diagnostic_ownership() -> None:
    report = vestra.Editor().validate(vestra.ProjectSnapshot.from_dict(VALID))
    assert isinstance(report.diagnostics, tuple)
    with pytest.raises(AttributeError):
        report.is_valid = False  # type: ignore[misc]
    with pytest.raises(AttributeError):
        vestra.PreflightOptions.for_validation().kind = "render"  # type: ignore[misc]


@pytest.mark.parametrize("value", [nan, inf, -inf])
def test_non_finite_dict_values_are_rejected(value: float) -> None:
    payload = dict(VALID)
    payload["metadata"] = {"value": value}
    with pytest.raises(ValueError):
        vestra.ProjectSnapshot.from_dict(payload)


def test_project_error_has_complete_base_contract(tmp_path: Path) -> None:
    with pytest.raises(vestra.ProjectError) as raised:
        vestra.ProjectSnapshot.load(tmp_path / "missing.json")
    error = raised.value
    assert error.kind == "project"
    assert isinstance(error.diagnostics, tuple)
    assert isinstance(error.warnings, tuple)
    assert error.warnings == ()
    assert error.diagnostics[0].code == "MVP-PROJECT-READ"


def test_preflight_options_keep_every_exposed_argument(tmp_path: Path) -> None:
    options = vestra.PreflightOptions.for_render(
        vestra.BackendPreference.CPU, tmp_path / "out.mp4", overwrite=True
    )
    assert options.kind == "render"
    assert options.backend == vestra.BackendPreference.CPU
    assert options.output == tmp_path / "out.mp4"
    assert options.overwrite


def test_every_preflight_constructor_preserves_its_exposed_state(tmp_path: Path) -> None:
    validation = vestra.PreflightOptions.for_validation()
    inspection = vestra.PreflightOptions.for_inspection()
    preparation = vestra.PreflightOptions.for_preparation(vestra.BackendPreference.CPU)
    rendering = vestra.PreflightOptions.for_render(
        vestra.BackendPreference.CPU, tmp_path / "out.mp4", overwrite=True
    )
    assert (validation.kind, validation.backend, validation.output, validation.overwrite) == ("validation", None, None, False)
    assert (inspection.kind, inspection.backend, inspection.output, inspection.overwrite) == ("inspection", None, None, False)
    assert (preparation.kind, preparation.backend, preparation.output, preparation.overwrite) == ("preparation", vestra.BackendPreference.CPU, None, False)
    assert (rendering.kind, rendering.backend, rendering.output, rendering.overwrite) == ("render", vestra.BackendPreference.CPU, tmp_path / "out.mp4", True)


def test_from_dict_accepts_general_mappings() -> None:
    assert vestra.ProjectSnapshot.from_dict(types.MappingProxyType(VALID)).to_dict()["schema_version"] == 3
    assert vestra.ProjectSnapshot.from_dict(UserDict(VALID)).to_dict()["schema_version"] == 3

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
    project = vestra.ProjectSnapshot.from_dict(source)
    del source
    assert project.to_dict()["metadata"] == {"nested": True}
