"""Verify the generated phrase remix as published media and master analysis."""

import json
from pathlib import Path
import runpy
import subprocess

import numpy as np
import pytest

from vestra import BackendPreference, Editor, PrepareOptions, ProjectSnapshot

EXAMPLE = Path(__file__).resolve().parents[1] / "examples/tutorials/phrase_remix.py"
RATE = 48_000


def test_phrase_remix_preserves_downbeats_and_drives_the_spectrum(tmp_path):
    example = runpy.run_path(str(EXAMPLE))
    source = tmp_path / "mix.wav"
    example["write_stereo_mix"](source)
    project = example["build_project"](source)

    snapshot = project.snapshot()
    inspection = Editor().inspect(snapshot)
    assert inspection.audio is not None
    assert [(clip.start, clip.end) for clip in inspection.audio.tracks[0].clips] == [
        (0, 2),
        (2, 3),
        (3, 4),
        (4, 4.5),
    ]

    output = tmp_path / "phrase-remix.mp4"
    result = project.render(output, backend="cpu", show_progress=False)
    assert result.duration_seconds == 4.5
    assert result.total_frames == 135
    assert result.selected_backend == "cpu"

    probe = subprocess.check_output(
        [
            "ffprobe",
            "-v",
            "error",
            "-count_frames",
            "-show_streams",
            "-of",
            "json",
            str(output),
        ]
    )
    streams = {stream["codec_type"]: stream for stream in json.loads(probe)["streams"]}
    audio = streams["audio"]
    assert (audio["codec_name"], int(audio["sample_rate"]), audio["channels"]) == (
        "aac",
        RATE,
        2,
    )
    video = streams["video"]
    assert (
        video["codec_name"],
        video["avg_frame_rate"],
        int(video["nb_read_frames"]),
    ) == (
        "h264",
        "30/1",
        135,
    )
    for stream in (audio, video):
        assert float(stream["start_time"]) == 0
        assert float(stream["duration"]) == pytest.approx(4.5, abs=1 / RATE)

    pcm = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(output),
            "-map",
            "0:a:0",
            "-f",
            "f32le",
            "-",
        ]
    )
    stereo = np.frombuffer(pcm, dtype="<f4").reshape(-1, 2)
    assert abs(len(stereo) - 4.5 * RATE) <= 1024  # AAC frame padding
    assert not np.allclose(stereo[:, 0], stereo[:, 1])

    def segment(start, end):
        return stereo[round(start * RATE) : round(end * RATE)]

    def rms(start, end):
        return float(np.sqrt(np.mean(segment(start, end) ** 2)))

    def spectrum(start, end):
        samples = segment(start, end)[:, 0]
        windowed = samples * np.hanning(len(samples))
        bins = np.fft.rfftfreq(4 * len(samples), 1 / RATE)
        magnitude = np.abs(np.fft.rfft(windowed, n=4 * len(samples)))
        return bins, magnitude / len(samples)

    def amplitude(start, end, low, high):
        bins, magnitude = spectrum(start, end)
        return float(magnitude[(bins >= low) & (bins <= high)].max())

    def peak(start, end, low, high):
        bins, magnitude = spectrum(start, end)
        band = (bins >= low) & (bins <= high)
        return float(bins[band][np.argmax(magnitude[band])])

    # The 82 Hz bass thins in the lead-in and returns on the chorus downbeat.
    bass = [
        amplitude(start, start + 0.095, 65, 112) for start in (0.03, 0.78, 1.53, 2.03)
    ]
    assert bass[0] > 2 * bass[1] > 4 * bass[2]
    assert bass[3] > 8 * bass[2]

    # Three later sixteenths keep the same stereo waveforms at each grid point.
    first = segment(1.53, 1.61)
    for repeat in (1.655, 1.78, 1.905):
        repeated = segment(repeat, repeat + 0.08)
        for channel in (0, 1):
            assert np.corrcoef(first[:, channel], repeated[:, channel])[0, 1] > 0.95

    assert peak(2.10, 2.37, 400, 560) == pytest.approx(494, abs=5)
    early = peak(3.27, 3.34, 130, 680)
    late = peak(3.47, 3.54, 130, 680)
    assert early > 1.5 * late
    assert rms(3.75, 3.96) < 0.001
    assert rms(4.00, 4.04) > 0.04

    authored = project.prepare(backend="cpu")
    for effect_type, frame in (
        ("filter_sweep", 35),
        ("beat_repeat", 54),
        ("pitch_shift", 70),
        ("tape_stop", 115),
    ):
        control = snapshot.to_dict()
        for clip in control["audio"]["tracks"][0]["clips"]:
            clip["effects"] = [
                effect
                for effect in clip.get("effects", [])
                if effect["type"] != effect_type
            ]
        baseline = Editor().prepare(
            ProjectSnapshot.from_dict(control, base_directory=snapshot.base_directory),
            PrepareOptions(backend=BackendPreference.CPU),
        )
        assert authored.render_frame_number(frame).to_bytes() != (
            baseline.render_frame_number(frame).to_bytes()
        ), f"{effect_type} must affect master analysis"
