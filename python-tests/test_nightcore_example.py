"""Exercise the generated remix through audio publication and master analysis."""

import json
from pathlib import Path
import runpy
import subprocess

import numpy as np
import pytest

from vestra import (
    BackendPreference,
    Editor,
    PrepareOptions,
    ProjectSnapshot,
)

EXAMPLE = Path(__file__).resolve().parents[1] / "examples/tutorials/nightcore_remix.py"
RATE = 48_000


def test_nightcore_example_publishes_remixed_audio_and_drives_spectrum(tmp_path):
    example = runpy.run_path(str(EXAMPLE))
    source = tmp_path / "mix.wav"
    example["write_stereo_mix"](source)
    project = example["build_project"](source, bass_boost=True)
    output = tmp_path / "remix.mp4"
    result = project.render(output, backend="cpu", show_progress=False)
    assert result.duration_seconds == 3
    assert result.total_frames == 90
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
    assert float(audio["duration"]) == pytest.approx(3, abs=1 / RATE)
    video = streams["video"]
    assert (
        video["codec_name"],
        video["avg_frame_rate"],
        int(video["nb_read_frames"]),
    ) == (
        "h264",
        "30/1",
        90,
    )
    assert float(video["duration"]) == pytest.approx(3)
    assert float(audio["start_time"]) == float(video["start_time"]) == 0

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
    assert abs(len(stereo) - 3 * RATE) <= 1024  # AAC frame padding

    def segment(start, end):
        return stereo[round(start * RATE) : round(end * RATE)]

    def rms(start, end):
        return float(np.sqrt(np.mean(segment(start, end) ** 2)))

    # Inspect the original 440 Hz note, before the first delayed repeat.
    note = segment(1.66, 1.78)[:, 0]
    frequencies = np.fft.rfftfreq(len(note), 1 / RATE)
    magnitudes = np.abs(np.fft.rfft(note * np.hanning(len(note))))
    band = (400 <= frequencies) & (frequencies <= 550)
    peak = frequencies[band][np.argmax(magnitudes[band])]
    assert peak == pytest.approx(484, abs=5)

    # The dry note has ended in each window. The wet track adds two quieter
    # repetitions, and only a short room tail follows the first source note.
    assert rms(0.21, 0.27) > 0.0001
    first_repeat, second_repeat = rms(1.93, 2.04), rms(2.20, 2.31)
    assert first_repeat > second_repeat > 0.003
    assert rms(2.65, 2.85) < 0.0001

    authored = project.prepare(backend="cpu")
    snapshot = project.snapshot()
    for frame, bypass in ((7, "room_reverb"), (67, "filtered_delay")):
        # Compare each tail while keeping the rest of the mix identical.
        control = snapshot.to_dict()
        music, throw = control["audio"]["tracks"]
        if bypass == "room_reverb":
            music["effects"] = [
                effect for effect in music["effects"] if effect["type"] != bypass
            ]
        else:
            throw["mute"] = True
        baseline = Editor().prepare(
            ProjectSnapshot.from_dict(control, base_directory=snapshot.base_directory),
            PrepareOptions(backend=BackendPreference.CPU),
        )
        assert authored.render_frame_number(frame).to_bytes() != (
            baseline.render_frame_number(frame).to_bytes()
        ), f"{bypass} must affect master analysis"
