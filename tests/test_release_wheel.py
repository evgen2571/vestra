"""Installed-wheel media assertions work without importing the native package."""

import importlib.util
from pathlib import Path
import struct
import subprocess

import pytest


def script():
    path = Path(__file__).resolve().parents[1] / "scripts/verify-wheel.py"
    spec = importlib.util.spec_from_file_location("verify_wheel", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.mark.parametrize("defect", [None, "count", "size", "timestamp"])
def test_decoded_output_checks_frames_dimensions_and_timestamps(monkeypatch, defect):
    frames = 1 if defect == "count" else 2
    width = 4 if defect == "size" else 2
    timestamp = 0.125 if defect == "timestamp" else 0.25
    stderr = (
        f"[showinfo] n:0 pts_time:0 s:{width}x2\n"
        f"[showinfo] n:1 pts_time:{timestamp} s:{width}x2\n"
    ).encode()
    monkeypatch.setattr(
        subprocess,
        "run",
        lambda *args, **kwargs: subprocess.CompletedProcess(
            args, 0, bytes(frames * 2 * 2 * 4), stderr
        ),
    )
    if defect is None:
        assert script().decode_frames(
            Path("output.mp4"), "ffmpeg", 2, 2, 2, 4
        ) == bytes(32)
    else:
        with pytest.raises(AssertionError):
            script().decode_frames(Path("output.mp4"), "ffmpeg", 2, 2, 2, 4)


def chunk(tag, content):
    return tag + struct.pack("<I", len(content)) + content + bytes(len(content) % 2)


def webp(frames):
    payload = b"WEBP" + b"".join(frames)
    return b"RIFF" + struct.pack("<I", len(payload)) + payload


def test_animated_webp_checks_timing_and_exposes_decodable_still_frames(tmp_path):
    image = chunk(b"VP8L", b"fixture")
    header = (
        bytes(6) + (1).to_bytes(3, "little") * 2 + (250).to_bytes(3, "little") + b"\x02"
    )
    path = tmp_path / "animation.webp"
    path.write_bytes(webp([chunk(b"ANMF", header + image)] * 2))
    assert script().webp_frames(path, 2, 2, 2, 4) == [webp([image])] * 2
    for arguments in [(4, 2, 2, 4), (2, 2, 3, 4), (2, 2, 2, 5)]:
        with pytest.raises(AssertionError):
            script().webp_frames(path, *arguments)


def test_animated_webp_rejects_truncated_container(tmp_path):
    path = tmp_path / "truncated.webp"
    path.write_bytes(webp([chunk(b"ANMF", bytes(16))])[:-1])
    with pytest.raises(AssertionError):
        script().webp_frames(path, 1, 1, 1, 4)
