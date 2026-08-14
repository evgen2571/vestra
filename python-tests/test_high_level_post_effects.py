import vestra
from vestra.effects import Bloom, Vignette
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
