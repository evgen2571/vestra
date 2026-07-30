import os
from pathlib import Path

import pytest

import video_editor


FIXTURE = Path("tests/fixtures/wgpu-small-rgba.json")


@pytest.mark.skipif(os.name == "nt", reason="uses a POSIX fake ffmpeg executable")
def test_encoder_failure_preserves_structured_render_context(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    fake_ffmpeg = fake_bin / "ffmpeg"
    fake_ffmpeg.write_text("#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then exit 0; fi\necho forced failure >&2\nexit 1\n")
    fake_ffmpeg.chmod(0o755)
    prepared = video_editor.Editor().prepare(
        video_editor.Project.load(FIXTURE),
        video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    )
    output = tmp_path / "encoder-failure.mp4"
    monkeypatch.setenv("PATH", f"{fake_bin}{os.pathsep}{os.environ['PATH']}")

    with pytest.raises(video_editor.RenderError) as captured:
        prepared.render_video(video_editor.PreparedVideoRenderRequest(output))

    error = captured.value
    assert error.kind == "render"
    assert isinstance(error.diagnostics, tuple)
    assert isinstance(error.warnings, tuple)
    assert isinstance(error.timings, video_editor.RenderTimings)
    assert isinstance(error.failure_context, video_editor.RenderFailureContext)
    assert isinstance(error.temporary_removed, bool)
    assert error.failure_context.stage is video_editor.RenderFailureStage.FRAME_WRITE
    assert error.temporary_removed is True
    assert not output.exists()
    assert list(path for path in tmp_path.iterdir() if path != fake_bin) == []
    with pytest.raises(video_editor.FrameRenderError) as invalidated:
        prepared.render_frame_number(0)
    assert invalidated.value.diagnostics[0].code == "MVP-PREPARED-INVALIDATED"
