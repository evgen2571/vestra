from pathlib import Path
import subprocess

import pytest

import vestra
from vestra.audio import (
    AudioEffectStack,
    AudioFadeCurve,
    AudioGainInterpolation,
    AudioGainKeyframe,
    BassBoost,
    ParametricEq,
    PlaybackSpeed,
)
from vestra.authoring import AuthoringError


def project(tmp_path: Path, *, output_audio: bool | None = None) -> vestra.Project:
    return vestra.Project(
        size=(2, 2),
        fps=1,
        duration=1,
        base_directory=tmp_path,
        output_audio=output_audio,
    )


def test_timeline_order_ids_snapshots_and_atomic_additions(tmp_path: Path) -> None:
    authored = project(tmp_path)
    assert authored.audio is authored.audio
    first = authored.audio.track("music")
    second = authored.audio.add_track(id="voice")
    automatic = authored.audio.add_track()
    assert [item.id for item in authored.audio.tracks] == [
        "music",
        "voice",
        "audio-track-000001",
    ]
    assert isinstance(authored.audio.tracks, tuple)
    assert isinstance(first.clips, tuple)
    before = authored.audio.tracks
    with pytest.raises(ValueError):
        authored.audio.track("music")
    with pytest.raises((TypeError, ValueError)):
        authored.audio.track("bad", gain=-1)
    assert authored.audio.tracks == before
    clip = first.add("tone.wav")
    with pytest.raises(ValueError):
        first.add("other.wav", id=clip.id)
    with pytest.raises(ValueError):
        first.add("other.wav", trim_end=0)
    assert first.clips == (clip,)
    assert second.clips == () and automatic.clips == ()


def test_clip_ids_are_global_and_foreign_ownership_is_rejected(tmp_path: Path) -> None:
    authored = project(tmp_path)
    left = authored.audio.track("left")
    right = authored.audio.track("right")
    first = left.add("tone.wav", id="clip")
    with pytest.raises(ValueError):
        right.add("tone.wav", id="clip")
    other = project(tmp_path)
    foreign = other.audio.track("foreign").add("tone.wav", trim_end=1)
    with pytest.raises(AuthoringError):
        authored.audio.crossfade(first, foreign)
    assert first.fade_out == 0


def test_audio_paths_deduplicate_lexically_without_construction_probe(
    tmp_path: Path,
) -> None:
    authored = project(tmp_path)
    track = authored.audio.track("music")
    track.add("a/../tone.wav", trim_end=1)
    track.add("tone.wav", start=0.1, trim_end=1)
    assert authored.snapshot().to_dict()["assets"] == [
        {
            "id": "audio-000001",
            "type": "audio",
            "source": "a/../tone.wav",
        }
    ]


@pytest.mark.parametrize(
    ("policy", "has_clip", "expected"),
    [
        (None, False, False),
        (None, True, True),
        (False, True, False),
        (True, False, True),
    ],
)
def test_output_audio_policy_matrix(
    tmp_path: Path, policy: bool | None, has_clip: bool, expected: bool
) -> None:
    authored = project(tmp_path, output_audio=policy)
    if has_clip:
        authored.audio.track("music").add("tone.wav", trim_end=1)
    assert authored.snapshot().to_dict()["output"]["audio"] is expected  # type: ignore[index]
    before = authored.output_audio
    with pytest.raises(TypeError):
        authored.output_audio = "yes"  # type: ignore[assignment]
    assert authored.output_audio is before


def test_clip_fields_and_fade_curves_lower_canonically(tmp_path: Path) -> None:
    authored = project(tmp_path)
    clip = authored.audio.track("music", mute=True, gain=0.75).add(
        "tone.wav",
        start=0.25,
        trim_start=0.1,
        trim_end=0.9,
        mute=True,
        gain=0.5,
        fade_in=0.2,
        fade_out=0.3,
        fade_in_curve=AudioFadeCurve.EQUAL_POWER,
        fade_out_curve=AudioFadeCurve.LINEAR,
    )
    item = authored.snapshot().to_dict()["audio"]["tracks"][0]["clips"][0]  # type: ignore[index]
    assert (
        item["start"] == 0.25 and item["trim_start"] == 0.1 and item["trim_end"] == 0.9
    )
    assert item["mute"] is True and item["gain"] == 0.5 and item["fade_in"] == 0.2
    assert item["fade_out"] == 0.3 and item["fade_in_curve"] == "equal_power"
    assert "fade_out_curve" not in item
    assert clip.track.id == "music"


def test_gain_automation_is_transactional_clearable_and_snapshot_stable(
    tmp_path: Path,
) -> None:
    authored = project(tmp_path)
    clip = authored.audio.track("music").add("tone.wav", trim_end=1)
    keyframes = [
        AudioGainKeyframe(0, 0, AudioGainInterpolation.LINEAR),
        AudioGainKeyframe(0.5, 1),
    ]
    clip.set_gain_automation(keyframes)
    keyframes.append(AudioGainKeyframe(1, 0.5))
    assert len(clip.gain_automation) == 2
    snapshot = authored.snapshot().to_dict()
    with pytest.raises(AuthoringError):
        clip.set_gain_automation([AudioGainKeyframe(0.5, 1)])
    assert len(clip.gain_automation) == 2
    with pytest.raises(TypeError):
        clip.set_gain_automation([object()])  # type: ignore[list-item]
    assert len(clip.gain_automation) == 2
    clip.clear_gain_automation()
    assert clip.gain_automation == ()
    assert "gain_automation" in snapshot["audio"]["tracks"][0]["clips"][0]  # type: ignore[index]


@pytest.mark.parametrize("curve", [AudioFadeCurve.EQUAL_POWER, AudioFadeCurve.LINEAR])
def test_crossfade_sets_computed_overlap_without_moving_or_trimming(
    tmp_path: Path, curve: AudioFadeCurve
) -> None:
    authored = project(tmp_path)
    track = authored.audio.track("music")
    outgoing = track.add("tone.wav", start=0, trim_start=0.1, trim_end=1.1)
    incoming = track.add("tone.wav", start=0.6, trim_start=0.2, trim_end=1.2)
    original = (
        outgoing.start,
        outgoing.trim_start,
        outgoing.trim_end,
        incoming.start,
        incoming.trim_start,
        incoming.trim_end,
    )
    authored.audio.crossfade(outgoing, incoming, curve=curve)
    assert outgoing.fade_out == incoming.fade_in == 0.4
    assert (
        outgoing.start,
        outgoing.trim_start,
        outgoing.trim_end,
        incoming.start,
        incoming.trim_start,
        incoming.trim_end,
    ) == original


def test_crossfade_failures_are_transactional(tmp_path: Path) -> None:
    authored = project(tmp_path)
    track = authored.audio.track("music")
    outgoing = track.add("tone.wav", trim_end=1)
    incoming = track.add("tone.wav", start=2, trim_end=3)
    with pytest.raises(AuthoringError):
        authored.audio.crossfade(outgoing, incoming)
    assert outgoing.fade_out == incoming.fade_in == 0
    with pytest.raises(AuthoringError):
        authored.audio.crossfade(outgoing, outgoing)
    with pytest.raises(TypeError):
        authored.audio.crossfade(outgoing, incoming, curve="linear")  # type: ignore[arg-type]


def test_effect_scopes_order_parameters_and_atomic_extend(tmp_path: Path) -> None:
    authored = project(tmp_path)
    track = authored.audio.track("music")
    clip = track.add("tone.wav", trim_end=1)
    clip.effects.add(ParametricEq(1000, 2, 1))
    clip.effects.add(BassBoost(5, 80))
    clip.effects.add(PlaybackSpeed(1.25))
    track.effects.add(ParametricEq(500, 1, 0.7))
    track.effects.add(BassBoost())
    authored.audio.effects.add(BassBoost(4, 60))
    authored.audio.effects.add(ParametricEq(200, -2, 0.8))
    before = track.effects.items
    with pytest.raises(ValueError):
        track.effects.add(PlaybackSpeed(1))
    with pytest.raises(ValueError):
        clip.effects.add(PlaybackSpeed(5))
    with pytest.raises(ValueError):
        track.effects.extend([BassBoost(), PlaybackSpeed(1)])
    assert track.effects.items == before
    data = authored.snapshot().to_dict()["audio"]
    assert [item["type"] for item in data["tracks"][0]["clips"][0]["effects"]] == [
        "parametric_eq",
        "bass_boost",
        "playback_speed",
    ]
    assert [item["type"] for item in data["tracks"][0]["effects"]] == [
        "parametric_eq",
        "bass_boost",
    ]
    assert [item["type"] for item in data["effects"]] == ["bass_boost", "parametric_eq"]
    assert data["effects"][0]["gain_db"] == 4.0
    assert all(
        item["id"]
        for item in data["effects"]
        + data["tracks"][0]["effects"]
        + data["tracks"][0]["clips"][0]["effects"]
    )


def test_audio_effect_extend_appends_each_value_once_in_order() -> None:
    stack = AudioEffectStack("master")
    values = [BassBoost(), ParametricEq(1_000, 2, 1)]
    stack.extend(values)
    assert stack.items == tuple(values)


def test_snapshot_is_independent_and_empty_master_effects_are_preserved(
    tmp_path: Path,
) -> None:
    authored = project(tmp_path)
    authored.audio.effects.add(BassBoost())
    first = authored.snapshot()
    before = first.to_dict()
    authored.audio.track("music").add("tone.wav", trim_end=1)
    authored.audio.tracks[0].clips[0].gain = 0.5
    assert first.to_dict() == before
    assert authored.snapshot().to_dict() == authored.snapshot().to_dict()
    assert first.to_dict()["audio"]["effects"][0]["type"] == "bass_boost"  # type: ignore[index]


def test_high_level_audio_render_contains_ffmpeg_audio_stream(tmp_path: Path) -> None:
    authored = vestra.Project(size=(2, 2), fps=1, duration=1, base_directory=Path("."))
    authored.audio.track("tone").add("examples/assets/tone.wav", trim_end=1)
    output = tmp_path / "high-level-audio.mp4"
    result = authored.render(output, backend="cpu", overwrite=True)
    assert output.is_file() and output.stat().st_size > 0
    assert result.audio_present is True
    probe = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
            str(output),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    assert set(probe.stdout.split()) == {"audio", "video"}


def test_public_audio_imports() -> None:
    from vestra import AudioClip, AudioEffectStack, AudioTimeline, AudioTrack
    from vestra.audio import AudioClip as ModuleClip

    assert AudioTimeline is vestra.audio.AudioTimeline
    assert (AudioClip, AudioTrack, AudioEffectStack) == (
        ModuleClip,
        vestra.audio.AudioTrack,
        vestra.audio.AudioEffectStack,
    )
