import wave
from pathlib import Path

import vestra
from vestra.effects import Bloom, Brightness, Vignette
from vestra.lowering import LoweringContext
from vestra.sources import Color


def _project() -> vestra.Project:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    project.root.add(Color("#204060"))
    return project


def test_root_post_effects_lower_without_audio_lowering(monkeypatch) -> None:
    project = _project()
    project.post_effects.add(Bloom(0.5, 2.0, 1.0))

    monkeypatch.setattr(LoweringContext, "lower_audio", lambda self, timeline: None)

    effects = project.snapshot().to_dict()["visual"]["post_effects"]
    assert [effect["type"] for effect in effects] == ["bloom"]


def test_root_post_effects_are_lowered_once_alongside_audio() -> None:
    project = _project()
    project.post_effects.add(Vignette(0.2, 0.8, 0.3, "#000000"))

    snapshot = project.snapshot().to_dict()

    assert [effect["type"] for effect in snapshot["visual"]["post_effects"]] == [
        "vignette"
    ]
    assert "audio" not in snapshot


def test_global_effect_keyframes_use_explicit_project_span() -> None:
    for empty in (False, True):
        project = vestra.Project(size=(2, 2), fps=(30000, 1001), duration=10)
        if not empty:
            project.root.add(Color("#000000"), duration=1)
        effect = project.post_effects.add(Brightness(0))
        effect.amount.keyframe(5, 0.25)
        assert project.validate().is_valid
        prepared = project.prepare(backend="cpu")
        assert (
            prepared.render_frame_number(299).to_bytes() == bytes([64, 64, 64, 255]) * 4
        )

        effect.amount.keyframe(11, 0.5)
        assert any(
            diagnostic.code == "VESTRA-KEYFRAME-TIME"
            for diagnostic in project.validate().diagnostics
        )


def test_global_effects_follow_automatic_audio_span(tmp_path: Path) -> None:
    with wave.open(str(tmp_path / "tone.wav"), "wb") as audio:
        audio.setparams((1, 2, 8000, 0, "NONE", "not compressed"))
        audio.writeframes(bytes(16000))
    project = vestra.Project(
        size=(2, 2),
        fps=(30000, 1001),
        duration=None,
        base_directory=tmp_path,
        output_audio=False,
    )
    project.root.add(Color("#000000"), duration=1)
    project.audio.track("music", mute=True).add("tone.wav", start=9)
    effect = project.post_effects.add(Brightness(0))
    effect.amount.keyframe(5, 0.25)
    assert project.validate().is_valid
    assert (
        project.prepare(backend="cpu").render_frame_number(299).to_bytes()
        == bytes([64, 64, 64, 255]) * 4
    )
