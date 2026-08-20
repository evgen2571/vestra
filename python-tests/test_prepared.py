import base64
from math import inf, nan
from pathlib import Path

import pytest

import vestra


FIXTURE = Path("tests/fixtures/wgpu-small-rgba.json")
PIXELS_A = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAF0lEQVR4nGP4z8DwHwgbGIC0w////xkAQBgHuub96GQAAAAASUVORK5CYII="
)
PIXELS_B = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFklEQVR4nGNgYGD4//8/w38wBaT/AwBJyAn3c3mLpwAAAABJRU5ErkJggg=="
)


def prepared() -> vestra.PreparedProject:
    return vestra.Editor().prepare(
        vestra.ProjectSnapshot.load(FIXTURE),
        vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )


def cpu_options() -> vestra.PrepareOptions:
    return vestra.PrepareOptions(backend=vestra.BackendPreference.CPU)


def image_project(tmp_path: Path, image_name: str, *, duration: int = 1) -> vestra.ProjectSnapshot:
    return vestra.ProjectSnapshot.from_dict(
        {
            "schema_version": 3,
            "output": {
                "path": "unused.mp4",
                "width": 2,
                "height": 2,
                "frame_rate": "1/1",
                "background": "#0000ff",
                "quality": "preview",
                "audio": False,
                "duration_mode": "explicit",
                "duration": duration,
            },
            "assets": [{"id": "image", "type": "image", "source": image_name}],
            "visual": {
                "clips": [{
                    "id": "image-layer",
                    "source": {"type": "image", "asset": "image"},
                    "start": 0,
                    "duration": duration,
                    "layer": 0,
                    "sizing": {"mode": "fit"},
                    "transform": {
                        "position": {"base_value": {"x": 0.5, "y": 0.5}},
                        "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                        "scale": {"base_value": {"x": 1, "y": 1}},
                    },
                    "opacity": {"base_value": 1},
                }]
            },
        },
        base_directory=tmp_path,
    )


def multi_frame_project(tmp_path: Path) -> vestra.ProjectSnapshot:
    return vestra.ProjectSnapshot.from_dict(
        {
            "schema_version": 3,
            "output": {
                "path": "unused.mp4",
                "width": 2,
                "height": 2,
                "frame_rate": "1/1",
                "background": "#102030",
                "quality": "preview",
                "audio": False,
                "duration_mode": "explicit",
                "duration": 18,
            },
            "assets": [],
            "visual": {"clips": []},
        },
        base_directory=tmp_path,
    )


def report_values(report: vestra.PreparationReport) -> tuple[object, ...]:
    timings = report.timings
    adapter = report.adapter
    fallback = report.fallback
    return (
        report.requested_backend, report.selected_backend,
        None if fallback is None else (fallback.code, fallback.stage, fallback.message),
        None if adapter is None else (
            adapter.adapter_name, adapter.device_type, adapter.graphics_backend,
            adapter.driver_name, adapter.driver_info, adapter.vendor_id, adapter.device_id,
        ),
        report.width, report.height,
        (report.frame_rate.numerator, report.frame_rate.denominator),
        report.duration_ns, report.duration_seconds, report.frame_count,
        report.decoded_asset_count, tuple(
            (
                warning.code, warning.category, warning.severity, warning.message,
                warning.pointer, warning.hint, warning.related_id,
            )
            for warning in report.warnings
        ),
        (
            timings.semantic_validation_ms, timings.preflight_ms, timings.plan_compile_ms,
            timings.asset_decode_ms, timings.backend_initialization_ms, timings.total_ms,
        ),
        report.supports_single_frame_rendering,
    )


def test_prepare_report_and_owned_frame() -> None:
    value = prepared()
    report = value.preparation_report
    assert report.requested_backend is vestra.BackendPreference.CPU
    assert report.selected_backend is vestra.BackendKind.CPU
    assert (report.width, report.height, report.frame_count) == (174, 130, 1)
    assert report.duration_ns == 1_000_000_000
    assert report.frame_rate.numerator == report.frame_rate.denominator == 1

    frame = value.render_frame_number(0)
    pixels = frame.to_bytes()
    assert bytes(frame) == pixels
    assert len(pixels) == frame.width * frame.height * 4
    assert frame.pixel_format is vestra.PixelFormat.RGBA8
    assert frame.timestamp_ns == 0
    assert value.render_frame_ns(0).frame_number == 0
    del value
    assert frame.to_bytes() == pixels


def test_frame_numbers_and_timestamps_validate_before_rendering() -> None:
    value = prepared()
    with pytest.raises(ValueError):
        value.render_frame_number(-1)
    with pytest.raises(TypeError):
        value.render_frame_number(True)  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        value.render_frame_number(2**80)
    with pytest.raises(vestra.FrameRenderError) as raised:
        value.render_frame_number(1)
    assert raised.value.diagnostics[0].code == "VESTRA-FRAME-RANGE"
    assert value.render_frame_number(0).frame_number == 0

    with pytest.raises(vestra.FrameRenderError) as raised:
        value.render_frame_number(2**64 - 1)
    assert raised.value.diagnostics[0].code == "VESTRA-FRAME-RANGE"

    with pytest.raises(ValueError):
        value.render_frame_ns(-1)
    with pytest.raises(TypeError):
        value.render_frame_ns(True)  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        value.render_frame_ns(2**130)
    with pytest.raises(vestra.FrameRenderError) as raised:
        value.render_frame_ns(value.preparation_report.duration_ns)
    assert raised.value.diagnostics[0].code == "VESTRA-FRAME-RANGE"
    assert value.render_frame_number(0).frame_number == 0


def test_seconds_path_uses_truncated_nanoseconds_and_rejects_non_finite() -> None:
    value = prepared()
    assert value.render_frame_seconds(0.999_999_999_9).frame_number == 0
    for seconds in (-1.0, nan, inf, -inf):
        with pytest.raises(ValueError):
            value.render_frame_seconds(seconds)
    with pytest.raises(TypeError):
        value.render_frame_seconds(True)  # type: ignore[arg-type]


def test_frame_rate_is_normalized_and_read_only() -> None:
    rate = vestra.FrameRate(60_000, 2_002)
    assert (rate.numerator, rate.denominator) == (30_000, 1_001)
    for numerator, denominator in ((0, 1), (1, 0)):
        with pytest.raises(ValueError):
            vestra.FrameRate(numerator, denominator)
    with pytest.raises(TypeError):
        vestra.FrameRate(True)
    with pytest.raises(AttributeError):
        rate.numerator = 24  # type: ignore[misc]


def test_prepared_values_are_immutable() -> None:
    value = prepared()
    frame = value.render_frame_number(0)
    with pytest.raises(AttributeError):
        value.preparation_report = value.preparation_report  # type: ignore[misc]
    with pytest.raises(AttributeError):
        frame.width = 1  # type: ignore[misc]
    with pytest.raises(AttributeError):
        vestra.PrepareOptions().backend = vestra.BackendPreference.CPU  # type: ignore[misc]


def test_fractional_frame_rate_uses_native_canonical_timestamp_boundaries(tmp_path: Path) -> None:
    project = vestra.ProjectSnapshot.from_dict(
        {
            "schema_version": 3,
            "output": {
                "path": "unused.mp4",
                "width": 2,
                "height": 2,
                "frame_rate": "30000/1001",
                "background": "#102030",
                "quality": "preview",
                "audio": False,
                "duration_mode": "explicit",
                "duration": 1,
            },
            "assets": [],
            "visual": {"clips": []},
        },
        base_directory=tmp_path,
    )
    value = vestra.Editor().prepare(
        project,
        vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    frame = value.render_frame_number(17)
    assert frame.timestamp_ns == 567_233_334
    assert value.render_frame_ns(frame.timestamp_ns).frame_number == 17
    assert value.render_frame_ns(frame.timestamp_ns - 1).frame_number == 16


def test_preparation_failure_for_missing_asset_keeps_structured_diagnostics(tmp_path: Path) -> None:
    project = image_project(tmp_path, "missing.png")
    assert vestra.Editor().validate(project).is_valid
    with pytest.raises(vestra.PreparationError) as captured:
        vestra.Editor().prepare(project, cpu_options())
    error = captured.value
    assert isinstance(error.kind, str)
    assert isinstance(error.diagnostics, tuple)
    assert isinstance(error.warnings, tuple)
    assert error.diagnostics
    diagnostic = error.diagnostics[0]
    assert diagnostic.code
    assert diagnostic.category.value
    assert diagnostic.severity.value
    assert diagnostic.message


def test_non_monotonic_frame_access_is_reusable_and_keeps_report_stable(tmp_path: Path) -> None:
    editor = vestra.Editor()
    project = multi_frame_project(tmp_path)
    value = editor.prepare(project, cpu_options())
    report = value.preparation_report
    before = report_values(report)
    requested = [10, 2, 10, 0, 17, 1, 17, 5]
    frames = [value.render_frame_number(number) for number in requested]
    assert [frame.frame_number for frame in frames] == requested
    assert frames[0].to_bytes() == frames[2].to_bytes()
    assert frames[4].to_bytes() == frames[6].to_bytes()
    assert report_values(report) == before

    with pytest.raises(vestra.FrameRenderError):
        value.render_frame_number(report.frame_count)
    assert value.render_frame_number(0).frame_number == 0
    assert report_values(report) == before

    del editor
    del project
    assert report_values(report) == before


def test_report_is_unchanged_after_success_and_pre_submission_errors(tmp_path: Path) -> None:
    value = vestra.Editor().prepare(multi_frame_project(tmp_path), cpu_options())
    report = value.preparation_report
    before = report_values(report)
    assert value.render_frame_number(3).frame_number == 3
    assert value.render_frame_ns(3_000_000_000).frame_number == 3
    assert value.render_frame_number(3).frame_number == 3
    assert value.render_frame_number(1).frame_number == 1
    with pytest.raises(vestra.FrameRenderError):
        value.render_frame_number(report.frame_count)
    with pytest.raises(vestra.FrameRenderError):
        value.render_frame_ns(report.duration_ns)
    assert value.render_frame_number(0).frame_number == 0
    assert report_values(report) == before


def test_cpu_frame_bytes_have_known_rgba_layout_and_straight_alpha_composition(tmp_path: Path) -> None:
    image = tmp_path / "pixels.png"
    image.write_bytes(PIXELS_A)
    frame = vestra.Editor().prepare(image_project(tmp_path, image.name), cpu_options()).render_frame_number(0)
    pixels = frame.to_bytes()

    def pixel_at(x: int, y: int) -> tuple[int, int, int, int]:
        offset = (y * frame.width + x) * 4
        return (
            pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]
        )

    assert len(pixels) == frame.width * frame.height * 4
    assert (frame.width, frame.height) == (2, 2)
    assert pixel_at(0, 0) == (255, 0, 0, 255)
    assert pixel_at(1, 0) == (0, 128, 127, 255)
    assert pixel_at(0, 1) == (0, 0, 255, 255)
    assert pixel_at(1, 1) == (0, 0, 255, 255)


def test_prepared_visual_assets_are_a_snapshot_while_new_preparation_reads_replacement(tmp_path: Path) -> None:
    image = tmp_path / "snapshot.png"
    image.write_bytes(PIXELS_A)
    project = image_project(tmp_path, image.name)
    editor = vestra.Editor()
    old = editor.prepare(project, cpu_options())
    old_bytes = old.render_frame_number(0).to_bytes()
    image.write_bytes(PIXELS_B)
    assert old.render_frame_number(0).to_bytes() == old_bytes
    new = editor.prepare(project, cpu_options())
    assert new.render_frame_number(0).to_bytes() != old_bytes
