from pathlib import Path
import os
import shutil
import subprocess
import sys
import wave

import imageio_ffmpeg
import pytest

from vestra._ffmpeg import resolve_ffmpeg


def test_default_uses_packaged_binary_without_path(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("IMAGEIO_FFMPEG_EXE", raising=False)
    monkeypatch.setenv("PATH", "")
    binary = Path(resolve_ffmpeg())
    assert binary.is_absolute()
    assert binary.is_relative_to(Path(imageio_ffmpeg.__file__).parent.resolve())
    assert subprocess.run([binary, "-version"], capture_output=True).returncode == 0


def test_default_never_falls_back_to_system_binary(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("IMAGEIO_FFMPEG_EXE", raising=False)
    monkeypatch.setattr(imageio_ffmpeg, "__file__", str(tmp_path / "__init__.py"))
    assert resolve_ffmpeg() == str(tmp_path / "binaries" / "ffmpeg")

    import vestra

    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    assert project.render_frame(0, backend="cpu").width == 2
    with pytest.raises(vestra.RenderError):
        project.render(tmp_path / "unavailable.mp4", backend="cpu", show_progress=False)


@pytest.mark.parametrize("selection", ["absolute", "relative", "command"])
def test_override_resolves_to_absolute_path(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, selection: str
) -> None:
    original = Path(imageio_ffmpeg.get_ffmpeg_exe()).resolve()
    binary = tmp_path / ("custom ffmpeg.exe" if os.name == "nt" else "custom ffmpeg")
    shutil.copy2(original, binary)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setenv("PATH", str(tmp_path))
    override = {
        "absolute": str(binary),
        "relative": f"./{binary.name}",
        "command": binary.name,
    }
    monkeypatch.setenv("IMAGEIO_FFMPEG_EXE", override[selection])
    assert resolve_ffmpeg() == str(binary)


@pytest.mark.parametrize("selection", ["missing", "directory", "not-ffmpeg"])
def test_invalid_override_has_useful_error(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, selection: str
) -> None:
    import vestra

    binary = {
        "missing": str(tmp_path / "missing"),
        "directory": str(tmp_path),
        "not-ffmpeg": sys.executable,
    }
    monkeypatch.setenv("IMAGEIO_FFMPEG_EXE", binary[selection])
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    output = tmp_path / "invalid.mp4"
    report = vestra.Editor().preflight(
        project.snapshot(),
        vestra.PreflightOptions.for_render(vestra.BackendPreference.CPU, output),
    )
    assert not report.is_ready
    assert any(item.code == "VESTRA-ENCODER-UNAVAILABLE" for item in report.errors)
    with pytest.raises(vestra.RenderError) as raised:
        project.render(output, backend="cpu", show_progress=False)
    assert raised.value.diagnostics
    assert not output.exists()


def test_analysis_failure_preserves_preparation_diagnostics(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    import vestra

    tone = tmp_path / "audio.wav"
    with wave.open(str(tone), "wb") as output:
        output.setparams((1, 2, 8000, 0, "NONE", "not compressed"))
        output.writeframes(bytes(16000))
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    project.audio.track().add(tone)
    layer = project.root.add(vestra.Color("#ffffff"))
    layer.opacity.bind(project.audio.signal.rms(), operation="multiply")
    monkeypatch.setenv("IMAGEIO_FFMPEG_EXE", str(tmp_path / "missing-ffmpeg"))
    with pytest.raises(vestra.PreparationError) as raised:
        project.prepare(backend="cpu")
    assert raised.value.diagnostics[0].code == "VESTRA-AUDIO-ANALYSIS"


def test_import_and_semantic_validation_do_not_resolve_ffmpeg(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    import vestra

    monkeypatch.setenv("IMAGEIO_FFMPEG_EXE", "/missing/ffmpeg")
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    assert project.validate().is_valid
    assert (
        vestra.Editor()
        .preflight(project.snapshot(), vestra.PreflightOptions.for_inspection())
        .is_valid
    )
    assert project.render_frame(0, backend="cpu").width == 2
    assert (
        vestra.Editor()
        .preflight(
            project.snapshot(),
            vestra.PreflightOptions.for_preparation(vestra.BackendPreference.CPU),
        )
        .is_ready
    )
