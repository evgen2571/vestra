from __future__ import annotations

import pytest

from vestra import Project, ScalarProperty, Text, WordHighlight, WordTiming


def test_text_animation_lowers_clip_local_progress_and_unicode_word_offsets() -> None:
    progress = ScalarProperty(0)
    progress.keyframe(0, 0)
    progress.keyframe(0.4, 1)
    source = Text(
        "été été",
        reveal=progress,
        word_highlight=WordHighlight(
            "#ffa000", [WordTiming(0, 0.2, 0, 3), WordTiming(0.2, 0.4, 4, 7)]
        ),
    )
    progress.clear_keyframes()
    project = Project(size=(240, 100), fps=30, duration=1)
    project.root.add(source, start=0.25, duration=0.5)
    snapshot = project.snapshot().to_dict()
    clip = snapshot["visual"]["clips"][0]
    assert clip["start"] == 0.25
    assert clip["source"]["reveal"] == {
        "base_value": 0.0,
        "keyframes": [
            {"time": 0.0, "value": 0.0, "interpolation": "linear"},
            {"time": 0.4, "value": 1.0, "interpolation": "linear"},
        ],
    }
    assert clip["source"]["word_highlight"] == {
        "colour": "#ffa000",
        "words": [
            {"start": 0.0, "end": 0.2, "text_start": 0, "text_end": 3},
            {"start": 0.2, "end": 0.4, "text_start": 4, "text_end": 7},
        ],
    }


def test_text_animation_rejects_invalid_authoring_values() -> None:
    with pytest.raises(ValueError, match="start < end"):
        WordTiming(0.2, 0.2, 0, 2)
    with pytest.raises(ValueError, match="must not overlap"):
        WordHighlight("#ffa000", [WordTiming(0, 1, 0, 2), WordTiming(0.5, 1.5, 3, 5)])
    with pytest.raises(ValueError, match="within the text"):
        Text("é", word_highlight=WordHighlight("#ffa000", [WordTiming(0, 1, 0, 2)]))
    with pytest.raises(ValueError, match="maximum"):
        Text("Title", reveal=1.1)
    title = Text("Title")
    with pytest.raises(ValueError, match="minimum"):
        title.reveal.keyframe(0, -0.1)
