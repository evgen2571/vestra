"""The standalone example must export a usable placeholder and reject bad inputs."""

import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

import pytest

EXAMPLE = Path(__file__).resolve().parents[1] / "examples/code-to-video"


def load_example(name):
    path = EXAMPLE / f"{name}.py"
    assert path.is_file(), f"missing example module: {path.name}"
    spec = importlib.util.spec_from_file_location(f"code_to_video_{name}", path)
    module = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(EXAMPLE))
    try:
        spec.loader.exec_module(module)
    finally:
        sys.path.pop(0)
    return module


def test_missing_assets_fail_before_creating_prepared_files(tmp_path):
    support = load_example("support")
    work = tmp_path / "work"
    with pytest.raises(FileNotFoundError, match="forward-fly-above-snow-forest.mp4"):
        support.prepare_assets(tmp_path / "missing", work)
    assert not work.exists()


def test_recording_rejects_invalid_and_short_video(tmp_path):
    support = load_example("support")
    invalid = tmp_path / "invalid.mp4"
    invalid.write_bytes(b"not a recording")
    with pytest.raises(ValueError, match="recording"):
        support.check_recording(invalid)
    short = tmp_path / "short.mp4"
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=s=160x90:r=30:d=1",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            str(short),
        ],
        check=True,
    )
    with pytest.raises(ValueError, match="8 seconds"):
        support.check_recording(short)


def test_complete_long_recording_is_accepted(tmp_path):
    support = load_example("support")
    recording = tmp_path / "typing.mp4"
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=s=160x90:r=1:d=240",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            str(recording),
        ],
        check=True,
    )
    assert support.check_recording(recording) == 240


def test_preparation_keeps_the_end_of_the_recording_in_eight_seconds(tmp_path):
    support, main = load_example("support"), load_example("main")
    assets, work = tmp_path / "assets", tmp_path / "work"
    assets.mkdir()
    recording = assets / "typing.mp4"
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=16x16:r=1:d=24,drawbox=color=blue:t=fill:enable='gte(t,16)'",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            str(recording),
        ],
        check=True,
    )
    for name in (
        "forward-fly-above-snow-forest.mp4",
        "forward-fly-through-road-to-snow-valley.mp4",
    ):
        shutil.copyfile(recording, assets / name)
    font = (EXAMPLE.parents[1] / "tests/assets/VestraTest-Regular.ttf").read_bytes()
    for archive, member in (
        ("Cinzel.zip", "static/Cinzel-Regular.ttf"),
        ("Lobster_Two.zip", "LobsterTwo-Regular.ttf"),
    ):
        with zipfile.ZipFile(assets / archive, "w") as bundle:
            bundle.writestr(member, font)
            bundle.writestr("OFL.txt", "test fixture; not redistributed")
    identity = support.fingerprint(recording)
    support.prepare_assets(assets, work, recording)
    assert support.fingerprint(recording) == identity
    prepared = work / "prepared/recording.mp4"
    probe = json.loads(
        subprocess.run(
            [
                "ffprobe",
                "-v",
                "error",
                "-count_frames",
                "-show_streams",
                "-of",
                "json",
                str(prepared),
            ],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    )["streams"][0]
    assert (
        probe["width"],
        probe["height"],
        probe["avg_frame_rate"],
        probe["nb_read_frames"],
    ) == (1600, 900, "30/1", "240")
    assert float(probe["duration"]) == 8
    final = subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(prepared),
            "-vf",
            "select=eq(n\\,239),scale=1:1",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ],
        capture_output=True,
        check=True,
    ).stdout
    assert final[2] > 150 and final[0] < 50, (
        "the final blue section must survive retiming"
    )
    project = main.build_project(work, "prepared/recording.mp4")
    assert project.validate().is_valid
    assert "prepared/recording.mp4" in project.snapshot().to_json()
    frame = project.render_frame(9, backend="cpu")
    pixel = (800 * frame.width + 1209) * 4
    rgb = frame.to_bytes()[pixel : pixel + 3]
    assert rgb[2] > 150 and rgb[0] < 50, "the recording must appear inside the frame"


def test_placeholder_exports_valid_portable_json_without_a_recording(tmp_path):
    pytest.importorskip("vestra")
    main = load_example("main")
    project = main.build_project(tmp_path)
    assert project.validate().is_valid
    snapshot = project.snapshot(output="render.mp4")
    snapshot.save(tmp_path / "project.json")
    data = json.loads((tmp_path / "project.json").read_text())
    assert data["output"]["width"] == 1920
    assert data["output"]["height"] == 1080
    assert data["output"]["frame_rate"] == "30/1"
    assert data["output"]["duration"] == 12
    assert {asset["source"] for asset in data["assets"]} == {
        "prepared/forest.mp4",
        "prepared/valley.mp4",
        "prepared/Cinzel.ttf",
        "prepared/LobsterTwo.ttf",
    }
    assert all(not Path(asset["source"]).is_absolute() for asset in data["assets"])
    assert "recording.mp4" not in snapshot.to_json()
    assert "Neovim" in snapshot.to_json()
    from vestra import Editor, ProjectSnapshot

    assert Editor().validate(ProjectSnapshot.load(tmp_path / "project.json")).is_valid


def test_later_footage_cannot_cover_the_editor_frame(tmp_path):
    pytest.importorskip("vestra")
    main = load_example("main")
    prepared = tmp_path / "prepared"
    prepared.mkdir()
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x58c18b:s=16x16:r=30:d=7",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            str(prepared / "forest.mp4"),
        ],
        check=True,
    )
    shutil.copyfile(prepared / "forest.mp4", prepared / "valley.mp4")
    for name in ("Cinzel", "LobsterTwo"):
        shutil.copyfile(
            EXAMPLE.parents[1] / "tests/assets/VestraTest-Regular.ttf",
            prepared / f"{name}.ttf",
        )
    frame = main.build_project(tmp_path).render_frame(9, backend="cpu")
    pixel = (800 * frame.width + 1600) * 4
    assert frame.to_bytes()[pixel : pixel + 4] == bytes((16, 28, 37, 255))
    background = (900 * frame.width + 100) * 4
    rgb = frame.to_bytes()[background : background + 3]
    assert max(rgb) - min(rgb) < 50, "the later shot must retain the muted grade"
