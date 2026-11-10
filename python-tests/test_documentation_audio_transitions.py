"""Check the controlled inputs and public contracts used by the media studies."""

from array import array
import importlib.util
from pathlib import Path
import sys
import wave

import pytest
from vestra import Crossfade

ROOT = Path(__file__).resolve().parents[1]


def load_example(name):
    path = ROOT / f"examples/tutorials/{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_generated_phrase_has_silence_and_five_distinct_onsets(tmp_path):
    example = load_example("audio_response")
    path = tmp_path / "pulses.wav"
    example.write_pulses(path)
    with wave.open(str(path), "rb") as recording:
        assert recording.getparams()[:4] == (1, 2, 48_000, 192_000)
        samples = array("h", recording.readframes(recording.getnframes()))
    if sys.byteorder != "little":
        samples.byteswap()
    assert max(abs(value) for value in samples) < 32767
    for onset in example.PULSE_STARTS:
        before = samples[int((onset - 0.05) * 48_000) : int(onset * 48_000)]
        after = samples[int(onset * 48_000) : int((onset + 0.05) * 48_000)]
        assert not any(before)
        assert max(abs(value) for value in after) > 3000


def test_audio_studies_validate_and_keep_analysis_when_muted(tmp_path):
    example = load_example("audio_response")
    path = tmp_path / "pulses.wav"
    example.write_pulses(path)
    for study in ("bounded", "smoothing", "spectra"):
        project = example.build_project(path, study)
        assert project.validate().is_valid
        assert project.output_audio is False
        snapshot = project.snapshot().to_dict()
        assert snapshot["output"]["audio"] is False
        assert len(snapshot["audio"]["tracks"][0]["clips"]) == 1
        assert project.validate().is_valid
    with pytest.raises(ValueError, match="Unknown study"):
        example.build_project(path, "missing")


def test_transition_comparisons_change_only_the_definition():
    example = load_example("transition_comparisons")
    snapshots = []
    for _, _, definition in example.definitions():
        project = example.build_project(definition, "Comparison")
        assert project.validate().is_valid
        visual = project.snapshot().to_dict()["visual"]
        assert len(visual["transitions"]) == 1
        placement = visual["transitions"][0]
        assert (placement["start"], placement["duration"]) == (1, 1)
        snapshots.append(visual["clips"])
    assert all(clips == snapshots[0] for clips in snapshots)


def test_documented_transition_fit_and_conflict_failures():
    example = load_example("transition_comparisons")
    project = example.build_project(Crossfade(), "Fit")
    outgoing, incoming = project.root.layers[:2]
    project.root.transitions.remove(project.root.transitions.items[0])
    project.root.transitions.add(outgoing, incoming, Crossfade(), start=0.5, duration=1)
    assert "VESTRA-TRANSITION-FIT" in {
        item.code for item in project.validate().diagnostics
    }

    project = example.build_project(Crossfade(), "Conflict")
    outgoing, incoming = project.root.layers[:2]
    project.root.transitions.add(
        outgoing, incoming, Crossfade(), start=1.25, duration=0.5
    )
    assert "VESTRA-TRANSITION-CONFLICT" in {
        item.code for item in project.validate().diagnostics
    }


@pytest.mark.parametrize("study", ("bounded", "smoothing", "spectra"))
def test_silent_output_preserves_actual_analysis_frames(tmp_path, study):
    example = load_example("audio_response")
    path = tmp_path / "pulses.wav"
    example.write_pulses(path)
    project = example.build_project(path, study)
    silent = project.prepare(backend="cpu")
    project.output_audio = True
    audible = project.prepare(backend="cpu")
    for time in (0, 0.4, 0.8, 2.8, 3.96):
        assert (
            silent.render_frame_seconds(time).to_bytes()
            == audible.render_frame_seconds(time).to_bytes()
        )


def test_measured_response_curves_show_attack_release_and_match_rendered_lines(
    tmp_path,
):
    example = load_example("audio_response")
    path = tmp_path / "pulses.wav"
    example.write_pulses(path)
    raw, fast, slow = example.sample_responses(path)
    assert len(raw) == example.FPS * example.DURATION
    onset = 11
    release = 24
    assert raw[onset] > fast[onset] > slow[onset]
    assert fast[release] > raw[release]
    assert slow[release] > raw[release]
    assert raw[release] < 0.001

    project = example.build_project(path, "smoothing")
    for layer in project.root.layers:
        if layer.id.startswith("dot-") or layer.id.endswith("playhead"):
            layer.visible = False
    pixels = project.prepare(backend="cpu").render_frame_seconds(0).to_bytes()
    for values, top, colour in zip(
        (raw, fast, slow), (160, 244, 328), (example.RAW, example.FAST, example.SLOW)
    ):
        expected = bytes.fromhex(colour[1:])
        for index in (11, 12, 18, 24, 36, 42):
            x = round(
                example.LEFT
                + index
                / (example.FPS * example.DURATION)
                * (example.RIGHT - example.LEFT)
            )
            y = round(top + 44 * (1 - values[index]))
            nearby = [
                pixels[(row * 640 + column) * 4 : (row * 640 + column) * 4 + 3]
                for row in range(y - 2, y + 3)
                for column in range(x - 2, x + 3)
            ]
            assert any(
                max(abs(a - b) for a, b in zip(pixel, expected)) <= 18
                for pixel in nearby
            ), (index, top, x, y)


def test_measured_curve_values_match_the_bound_circle_size(tmp_path):
    example = load_example("audio_response")
    path = tmp_path / "pulses.wav"
    example.write_pulses(path)
    fast = example.sample_responses(path)[1]
    prepared = example.build_project(path, "bounded").prepare(backend="cpu")
    for index in (0, 12, 24, 36, 84, 119):
        pixels = prepared.render_frame_number(index).to_bytes()
        filled = [
            x
            for x in range(80, 270)
            if pixels[(218 * 640 + x) * 4 : (218 * 640 + x) * 4 + 3]
            == bytes.fromhex(example.FAST[1:])
        ]
        radius = (max(filled) - min(filled) + 1) / 2
        assert radius == pytest.approx(62 * (0.75 + 0.55 * fast[index]), abs=1.5)


def test_spectrum_layouts_render_the_same_band_values(tmp_path):
    import math

    example = load_example("audio_response")
    path = tmp_path / "pulses.wav"
    example.write_pulses(path)
    project = example.build_project(path, "spectra")
    sources = [
        layer.source
        for layer in project.root.layers
        if layer.id.startswith("spectrum-")
    ]
    for field in (
        "band_count",
        "min_hz",
        "max_hz",
        "sensitivity",
        "attack_seconds",
        "release_seconds",
        "gradient",
        "min_bar_height_ratio",
    ):
        assert getattr(sources[0], field) == getattr(sources[1], field)
    prepared = project.prepare(backend="cpu")

    def is_bar(pixels, x, y):
        return min(pixels[(y * 640 + x) * 4 : (y * 640 + x) * 4 + 3]) > 80

    distributions = []
    for time in (0.4, 2.8):
        pixels = prepared.render_frame_seconds(time).to_bytes()
        amplitudes = []
        for band in range(16):
            x = round(32 + (band + 0.5) * 304 / 16)
            linear = sum(is_bar(pixels, x, y) for y in range(195, 369)) / 174
            angle = (band + 0.5) * math.tau / 16
            radii = [
                radius
                for radius in range(51, 115)
                if is_bar(
                    pixels,
                    round(493 + math.sin(angle) * radius),
                    round(294 - math.cos(angle) * radius),
                )
            ]
            radial = (max(radii, default=50) + 0.5 - 50.6) / 64.4
            assert linear == pytest.approx(radial, abs=0.04), (time, band)
            amplitudes.append(linear)
        distributions.append(amplitudes)
    assert (
        distributions[0][1] > 0.5 and distributions[1][1] > 0.5
    )  # The recurring bass.
    assert distributions[0][4] > distributions[1][4] + 0.2
    assert distributions[1][11] > distributions[0][11] + 0.2


@pytest.mark.parametrize("study", ("bounded", "smoothing", "spectra"))
def test_audio_studies_reserve_a_clear_mobile_control_band(tmp_path, study):
    example = load_example("audio_response")
    path = tmp_path / "pulses.wav"
    example.write_pulses(path)
    frame = (
        example.build_project(path, study)
        .prepare(backend="cpu")
        .render_frame_seconds(1.2)
    )
    assert (frame.width, frame.height) == (640, 560)
    assert frame.to_bytes()[442 * 640 * 4 :] == bytes.fromhex("0b141cff") * 640 * 118
