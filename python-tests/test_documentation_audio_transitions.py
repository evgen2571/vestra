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
    path = ROOT / f"docs/assets/examples/{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_generated_pulses_have_silence_and_five_audible_onsets(tmp_path):
    example = load_example("audio_response")
    path = tmp_path / "pulses.wav"
    example.write_pulses(path)
    with wave.open(str(path), "rb") as recording:
        assert recording.getparams()[:4] == (1, 2, 48_000, 144_000)
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
        project.output_audio = False
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


def test_smoothed_circle_has_a_reduced_onset_and_a_visible_release(tmp_path):
    example = load_example("audio_response")
    path = tmp_path / "pulses.wav"
    example.write_pulses(path)
    prepared = example.build_project(path, "smoothing").prepare(backend="cpu")

    def circle_areas(time):
        pixels = prepared.render_frame_seconds(time).to_bytes()
        areas = [0, 0, 0]
        for offset in range(0, len(pixels), 4):
            red, green, blue = pixels[offset : offset + 3]
            if 100 < red < 200 and 180 < green < 245 and 150 < blue < 230:
                x = (offset // 4) % 640
                areas[min(x // 213, 2)] += 1
        return areas

    _, fast_onset, slow_onset = circle_areas(0.3)
    raw_release, fast_release, slow_release = circle_areas(0.55)
    assert fast_onset > slow_onset
    assert fast_release > raw_release
    assert slow_release > raw_release
