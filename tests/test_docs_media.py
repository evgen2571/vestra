"""Exercise media verification against tiny, real encoded outputs."""

from pathlib import Path
import runpy
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]
VERIFY = runpy.run_path(str(ROOT / "scripts/render-docs-media.py"))["verify_video"]


@pytest.fixture
def audible_video(tmp_path):
    output = tmp_path / "video.mp4"
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:size=32x18:rate=10:duration=0.4",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.4",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            str(output),
        ],
        check=True,
    )
    return output


def test_verifies_audio_and_actual_frame_count(audible_video):
    VERIFY(audible_video, (32, 18), seconds=0.4, fps=10, audio=True)


@pytest.mark.parametrize(
    "settings",
    [
        {"seconds": 0.5, "fps": 10, "audio": True},
        {"seconds": 0.4, "fps": 10, "audio": False},
        {"seconds": 0.4, "fps": 30, "audio": True},
    ],
)
def test_rejects_wrong_frame_or_audio_contract(audible_video, settings):
    with pytest.raises(AssertionError):
        VERIFY(audible_video, (32, 18), **settings)
