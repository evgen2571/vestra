import os
from pathlib import Path

import pytest

import vestra


def test_wgpu_prepared_frame_has_owned_cpu_bytes_when_adapter_is_available() -> None:
    project = vestra.ProjectSnapshot.load(Path("tests/fixtures/wgpu-small-rgba.json"))
    try:
        prepared = vestra.Editor().prepare(
            project,
            vestra.PrepareOptions(backend=vestra.BackendPreference.WGPU),
        )
    except vestra.PreparationError as error:
        unavailable = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}
        if os.environ.get("VESTRA_REQUIRE_WGPU") == "1" or any(
            diagnostic.code not in unavailable for diagnostic in error.diagnostics
        ):
            raise
        pytest.skip("no compatible WGPU adapter")

    report = prepared.preparation_report
    frame = prepared.render_frame_number(0)
    assert report.selected_backend is vestra.BackendKind.WGPU
    assert report.adapter is not None
    assert len(frame.to_bytes()) == frame.width * frame.height * 4
    retained = frame.to_bytes()
    assert prepared.render_frame_number(0).to_bytes() == retained
    del prepared
    assert frame.to_bytes() == retained


def test_wgpu_random_access_matches_cpu_metadata_when_adapter_is_available(tmp_path: Path) -> None:
    project = vestra.ProjectSnapshot.from_dict(
        {
            "schema_version": 2,
            "output": {
                "path": "unused.mp4", "width": 2, "height": 2,
                "frame_rate": "1/1", "background": "#102030", "quality": "preview",
                "audio": False, "duration_mode": "explicit", "duration": 18,
            },
            "assets": [],
            "visual": {"clips": []},
        },
        base_directory=tmp_path,
    )
    try:
        wgpu = vestra.Editor().prepare(
            project,
            vestra.PrepareOptions(backend=vestra.BackendPreference.WGPU),
        )
    except vestra.PreparationError as error:
        unavailable = {"WGPU-ADAPTER-NOT-FOUND", "WGPU-NO-COMPATIBLE-ADAPTER"}
        if os.environ.get("VESTRA_REQUIRE_WGPU") == "1" or any(
            diagnostic.code not in unavailable for diagnostic in error.diagnostics
        ):
            raise
        pytest.skip("no compatible WGPU adapter")

    cpu = vestra.Editor().prepare(
        project,
        vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    report = wgpu.preparation_report
    assert report.selected_backend is vestra.BackendKind.WGPU
    assert report.adapter is not None
    assert report.adapter.device_type.value in {
        "discretegpu", "integratedgpu", "virtualgpu", "cpu", "other",
    }
    assert report.adapter.graphics_backend.value in {
        "vulkan", "metal", "dx12", "gl", "browserwebgpu", "other",
    }
    requested = [10, 2, 10, 0, 17, 1, 17, 5]
    frames = [wgpu.render_frame_number(number) for number in requested]
    assert [frame.frame_number for frame in frames] == requested
    assert frames[0].to_bytes() == frames[2].to_bytes()
    assert frames[4].to_bytes() == frames[6].to_bytes()
    cpu_frame = cpu.render_frame_number(10)
    wgpu_frame = frames[0]
    assert (
        wgpu_frame.width, wgpu_frame.height, wgpu_frame.frame_number,
        wgpu_frame.timestamp_ns, wgpu_frame.pixel_format, len(wgpu_frame.to_bytes()),
    ) == (
        cpu_frame.width, cpu_frame.height, cpu_frame.frame_number,
        cpu_frame.timestamp_ns, cpu_frame.pixel_format, len(cpu_frame.to_bytes()),
    )
    retained = wgpu_frame.to_bytes()
    del wgpu
    assert wgpu_frame.to_bytes() == retained
