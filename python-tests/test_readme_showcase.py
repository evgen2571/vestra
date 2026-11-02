"""The silent hero still uses real audio for its spectrum and halo animation."""

import importlib.util
from pathlib import Path

from vestra import PreparedVideoRenderRequest


ROOT = Path(__file__).resolve().parents[1]


def test_silent_hero_reacts_to_audio_and_exports_video_only(tmp_path):
    spec = importlib.util.spec_from_file_location(
        "readme_showcase", ROOT / "examples/showcase/readme-demo/main.py"
    )
    scene = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(scene)
    project = scene.build_project((320, 180), 6)
    assert project.output_audio is False
    prepared = project.prepare(backend="cpu")
    loud = prepared.render_frame_seconds(8).to_bytes()
    between_beats = prepared.render_frame_seconds(8.25).to_bytes()

    def halo(frame):
        # This crop excludes the spectrum, captions and timeline progress line.
        return b"".join(
            frame[(y * 320 + 215) * 4 : (y * 320 + 259) * 4] for y in range(61, 105)
        )

    assert halo(loud) != halo(between_beats)
    project.audio.tracks[0].mute = True
    silent = project.prepare(backend="cpu").render_frame_seconds(8).to_bytes()
    assert halo(loud) != halo(silent), "the halo must respond to the audio input"

    def spectrum(frame):
        return b"".join(
            frame[(y * 320 + 263) * 4 : (y * 320 + 305) * 4] for y in range(50, 120)
        )

    assert spectrum(loud) != spectrum(silent)
    result = prepared.render_video(PreparedVideoRenderRequest(tmp_path / "hero.mp4"))
    assert result.audio_present is False
