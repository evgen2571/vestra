import os
from pathlib import Path

import pytest

import vestra


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
    prepared = vestra.Editor().prepare(
        vestra.ProjectSnapshot.load(FIXTURE),
        vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )
    output = tmp_path / "encoder-failure.mp4"
    monkeypatch.setenv("PATH", f"{fake_bin}{os.pathsep}{os.environ['PATH']}")

    with pytest.raises(vestra.RenderError) as captured:
        prepared.render_video(vestra.PreparedVideoRenderRequest(output))

    error = captured.value
    assert error.kind == "render"
    assert isinstance(error.diagnostics, tuple)
    assert isinstance(error.warnings, tuple)
    assert isinstance(error.timings, vestra.RenderTimings)
    assert isinstance(error.failure_context, vestra.RenderFailureContext)
    assert isinstance(error.temporary_removed, bool)
    assert error.failure_context.stage is vestra.RenderFailureStage.FRAME_WRITE
    assert error.temporary_removed is True
    assert not output.exists()
    assert list(path for path in tmp_path.iterdir() if path != fake_bin) == []
    with pytest.raises(vestra.FrameRenderError) as invalidated:
        prepared.render_frame_number(0)
    assert invalidated.value.diagnostics[0].code == "MVP-PREPARED-INVALIDATED"
