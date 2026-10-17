from __future__ import annotations

import shutil
import subprocess
from pathlib import Path
from typing import cast

import pytest

import vestra
from vestra.lowering import source_capabilities
from vestra.sources import Color, Video


def test_video_is_public_and_lowers_as_a_deduplicated_dynamic_source() -> None:
    assert vestra.Video is Video
    assert source_capabilities(Video("assets/clip.mp4")).has_intrinsic_duration

    project = vestra.Project(size=(16, 16), fps=2, duration=4)
    source = Video("assets/clip.mp4")
    first = project.root.add(source, source_start=1.5, duration=1, playback_rate=2)
    second = project.root.add(source, start=1, duration=1)
    data = project.snapshot().to_dict()

    assets = cast(list[dict[str, object]], data["assets"])
    visual = cast(dict[str, object], data["visual"])
    clips = cast(list[dict[str, object]], visual["clips"])
    assert len(assets) == 1
    assert assets[0]["type"] == "video"
    assert clips[0]["source_start"] == 1.5
    assert clips[0]["playback_rate"] == 2.0
    assert first.source is not second.source
    assert cast(Video, first.source).path == cast(Video, second.source).path


@pytest.fixture
def video_path(tmp_path: Path) -> Path:
    if shutil.which("ffmpeg") is None:
        pytest.skip("ffmpeg is required")
    path = tmp_path / "clip.mp4"
    subprocess.run(
        [
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=2x2:r=2:d=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            str(path),
        ],
        check=True,
    )
    return path


def test_video_omitted_duration_uses_native_intrinsic_duration(video_path: Path) -> None:
    project = vestra.Project(size=(2, 2), fps=2, duration=1)
    layer = project.root.add(Video(video_path), source_start=0.25, playback_rate=2)
    assert layer.duration == pytest.approx(0.375, abs=0.01)


def test_nested_video_preflight_identifies_canonical_clip(video_path: Path) -> None:
    project = vestra.Project(size=(2, 2), fps=vestra.FrameRate(30000, 1001), duration=10)
    project.root.add(Color("#000000"), duration=1)
    outer = project.root.group(start=3, duration=5, id="outer")
    outer.add(Color("#000000"), duration=1)
    inner = outer.group(start=1, duration=2, id="inner")
    inner.add(Color("#000000"), duration=1)
    inner.add(Video(video_path), start=0.5, duration=0.25,
              source_start=0.75, playback_rate=2, id="invalid-video")

    report = vestra.Editor().preflight(
        project.snapshot(), vestra.PreflightOptions.for_inspection()
    )
    assert not report.is_ready
    assert len(report.errors) == 1
    diagnostic = report.errors[0]
    assert diagnostic.code == "VESTRA-VIDEO-TIMING"
    assert diagnostic.pointer == "/visual/clips/1/source/clips/1/source/clips/1"
    assert "'outer/inner/invalid-video'" in diagnostic.message
