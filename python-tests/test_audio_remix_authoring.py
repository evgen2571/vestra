"""Round-trip the clip remix effects through both Python authoring paths."""

from copy import deepcopy
from pathlib import Path
import wave

import pytest

import vestra
from vestra.authoring import ProjectBuilder


def test_clip_remix_authoring_paths_match(tmp_path: Path) -> None:
    keys = [vestra.AudioFilterKeyframe(0, 40), vestra.AudioFilterKeyframe(0.25, 1200)]
    project = vestra.Project(size=(32, 24), fps=20, base_directory=tmp_path)
    track = project.audio.track()
    clip = track.add("tone.wav", start=0, trim_end=1)
    clip.effects.extend([
        vestra.BeatRepeat(slice_start=0.125, slice_duration=0.125, output_start=0.5, repeats=4),
        vestra.FilterSweep(mode="high_pass", keyframes=keys),
        vestra.PitchShift(semitones=2),
        vestra.TapeStop(start=0.75, duration=0.25),
    ])
    keys.clear()
    data = project.snapshot().to_dict()
    effects = data["audio"]["tracks"][0]["clips"][0]["effects"]
    assert [effect["type"] for effect in effects] == [
        "beat_repeat", "filter_sweep", "pitch_shift", "tape_stop",
    ]
    assert effects[1]["keyframes"] == [
        {"time": 0, "frequency_hz": 40}, {"time": 0.25, "frequency_hz": 1200},
    ]

    builder = ProjectBuilder(
        width=32, height=24, frame_rate=vestra.FrameRate(20, 1), output_path=tmp_path / "out.mp4",
        base_directory=tmp_path,
    )
    asset = builder.add_audio_asset("tone.wav")
    advanced = builder.audio.add_track().add_clip(asset=asset, start=0, trim_end=1).effects
    authored = [
        advanced.add_beat_repeat(slice_start=0.125, slice_duration=0.125, output_start=0.5, repeats=4),
        advanced.add_filter_sweep(
            mode="high_pass",
            keyframes=[vestra.AudioFilterKeyframe(0, 40), vestra.AudioFilterKeyframe(0.25, 1200)],
        ),
        advanced.add_pitch_shift(semitones=2),
        advanced.add_tape_stop(start=0.75, duration=0.25),
    ]
    advanced_data = [effect.to_canonical() for effect in authored]
    for item, expected in zip(advanced_data, effects, strict=True):
        assert item == {**expected, "id": item["id"]}
        params = {key: value for key, value in item.items() if key not in {"id", "type"}}
        generic = advanced.add_effect(item["type"], **params).to_canonical()
        assert generic == {**item, "id": generic["id"]}
    copied = advanced_data[1]
    copied["keyframes"].clear()
    assert authored[1].to_canonical()["keyframes"] == effects[1]["keyframes"]

    with pytest.raises(ValueError, match="scope"):
        builder.audio.effects.add_pitch_shift(semitones=2)
    with pytest.raises(ValueError, match="scope"):
        track.effects.add(vestra.TapeStop(start=0, duration=0.25))

    invalid = deepcopy(effects[1]["keyframes"])
    invalid[1]["time"] = 0.000001
    with pytest.raises(ValueError, match="48 kHz"):
        advanced.add_effect("filter_sweep", mode="high_pass", keyframes=invalid)


@pytest.mark.parametrize("keys", [
    [],
    [vestra.AudioFilterKeyframe(0.01, 40)],
    [vestra.AudioFilterKeyframe(0, 40), vestra.AudioFilterKeyframe(0, 1200)],
])
def test_filter_sweep_rejects_invalid_keyframe_times(keys: list[vestra.AudioFilterKeyframe]) -> None:
    with pytest.raises(ValueError, match="keyframes"):
        vestra.FilterSweep(mode="high_pass", keyframes=keys)

@pytest.mark.parametrize("trim_end", [None, 1])
def test_remix_bounds_follow_preceding_speed_with_explicit_trim_and_source_eof(
    tmp_path: Path, trim_end: int | None,
) -> None:
    with wave.open(str(tmp_path / "one-second.wav"), "wb") as source:
        source.setparams((2, 2, 48_000, 0, "NONE", "not compressed"))
        source.writeframes(bytes(48_000 * 4))

    project = vestra.Project(size=(32, 24), fps=20, base_directory=tmp_path)
    clip = project.audio.track().add("one-second.wav", start=0, trim_end=trim_end)
    clip.effects.extend([
        vestra.PlaybackSpeed(rate=2, link_pitch=True),
        vestra.BeatRepeat(slice_start=0, slice_duration=0.1, output_start=0.4, repeats=2),
    ])
    assert project.validate().is_valid == (trim_end is None)
    report = vestra.Editor().preflight(project.snapshot(), vestra.PreflightOptions.for_validation())
    assert not report.is_ready
    assert ("VESTRA-AUDIO-EFFECT-TIMING", "/audio/tracks/0/clips/0/effects/1/output_start") in [
        (diagnostic.code, diagnostic.pointer) for diagnostic in report.errors
    ]
