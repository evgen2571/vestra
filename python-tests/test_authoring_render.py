from pathlib import Path
import subprocess

import pytest

import vestra
from vestra import FrameRate
from vestra.authoring import ProjectBuilder, Sizing


def test_background_only_builder_renders_on_cpu(tmp_path: Path) -> None:
    project = ProjectBuilder(
        width=160,
        height=90,
        frame_rate=FrameRate(10, 1),
        output_path="canonical-output.mp4",
        duration=0.2,
        background="#ff0000",
        base_directory=tmp_path,
    ).build()
    output = tmp_path / "result.mp4"
    result = vestra.Editor().render(
        project,
        vestra.RenderRequest(
            output, backend=vestra.BackendPreference.CPU, overwrite=True,
        ),
    )
    assert result.output_path == output
    assert result.width == 160
    assert result.height == 90
    assert result.duration_seconds == 0.2
    assert result.total_frames == 2
    assert output.is_file() and output.stat().st_size > 0
    assert not list(tmp_path.glob("*.tmp"))
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
         "stream=width,height,nb_frames,duration", "-of", "default=noprint_wrappers=1", str(output)],
        check=True, capture_output=True, text=True,
    )
    assert "width=160" in probe.stdout
    assert "height=90" in probe.stdout
    assert "nb_frames=2" in probe.stdout
    assert "duration=0.200000" in probe.stdout


@pytest.mark.parametrize(
    "preset",
    ["classic", "dense", "neon", "mirror", "center_out", "circle", "neon_circle", "arc"],
)
def test_public_spectrum2d_preset_authoring_renders_on_cpu(tmp_path: Path, preset: str) -> None:
    project_builder = ProjectBuilder(
        width=64, height=64, frame_rate=FrameRate(10, 1), output_path="spectrum.mp4",
        duration=0.2, base_directory=Path(__file__).resolve().parents[1],
    )
    audio = project_builder.add_audio_asset("examples/assets/tone.wav")
    track = project_builder.audio.add_track(id="music")
    track.add_clip(asset=audio, start=0, trim_end=0.2)
    clip = project_builder.add_spectrum2d_clip(start=0, duration=0.2, layer=1, preset=preset)  # type: ignore[arg-type]
    if preset in {"neon", "neon_circle"}:
        assert [effect.kind for effect in clip.effects.items] == ["glow", "bloom"]
    project = project_builder.build()
    output = tmp_path / "spectrum.mp4"
    result = vestra.Editor().render(
        project, vestra.RenderRequest(
            output, backend=vestra.BackendPreference.CPU, overwrite=True,
        ),
    )
    assert result.output_path == output
    assert result.width == 64 and result.height == 64 and result.total_frames == 2
    assert output.is_file() and output.stat().st_size > 0


@pytest.mark.parametrize("preset", ["circle", "neon_circle"])
def test_public_spectrum2d_integration_project_exercises_audio_reactive_background_and_bloom(
    preset: str,
) -> None:
    def prepare_project(with_spectrum: bool):
        builder = ProjectBuilder(
            width=64, height=54, frame_rate=FrameRate(10, 1), output_path="spectrum.mp4",
            duration=3.0, background="#101018", base_directory=Path(__file__).resolve().parents[1],
        )
        image = builder.add_image_asset("examples/assets/green.png")
        background = builder.add_image_clip(
            source=image, start=0, duration=3, layer=0, sizing=Sizing.cover(),
        )
        background.transform.scale.react_to(
            builder.audio.master.rms().remap(0, 1, 1, 1.08).envelope(0.02, 0.15),
        )
        audio = builder.add_audio_asset("examples/assets/tone.wav")
        track = builder.audio.add_track(id="music")
        # Keep the reactive bands above the valid low-energy/no-bars case.
        track.add_clip(asset=audio, start=0, trim_end=3, gain=2.0)
        spectrum = None
        if with_spectrum:
            spectrum = builder.add_spectrum2d_clip(
                start=0, duration=3, layer=1, preset=preset,
            )

            canonical = builder.to_dict()
            spectrum_data = next(
                clip for clip in canonical["visual"]["clips"] if clip["id"] == spectrum.id
            )
            assert spectrum_data["source"]["type"] == "spectrum2d"
            assert "preset" not in spectrum_data
            expected_effects = ["glow", "bloom"] if preset == "neon_circle" else []
            assert [effect["type"] for effect in spectrum_data["effects"]] == expected_effects
        assert builder.validate().is_valid
        return vestra.Editor().prepare(
            builder.build(),
            vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
        )

    baseline = prepare_project(with_spectrum=False)
    spectrum = prepare_project(with_spectrum=True)
    assert baseline.preparation_report.timings.audio_analysis_ms > 0
    assert spectrum.preparation_report.timings.audio_analysis_ms > 0

    # Frame 10 is inside the bundled tone, so the configured Spectrum2D region
    # must differ from the otherwise identical audio-reactive baseline.
    baseline_frame = baseline.render_frame_number(10).to_bytes()
    spectrum_frame = spectrum.render_frame_number(10).to_bytes()
    assert baseline_frame != spectrum_frame
    region_differs = any(
        baseline_frame[(y * 64 + x) * 4:(y * 64 + x + 1) * 4]
        != spectrum_frame[(y * 64 + x) * 4:(y * 64 + x + 1) * 4]
        for y in range(11, 43)
        for x in range(12, 52)
    )
    assert region_differs
