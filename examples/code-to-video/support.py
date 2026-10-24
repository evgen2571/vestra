"""Local asset preparation, editor frame and command-line entry point."""

import argparse
import hashlib
import json
from math import isfinite
from pathlib import Path
import subprocess
import zipfile

from vestra.sources import Circle, Rectangle, Text, Video

ROOT = Path(__file__).resolve().parents[2]
SOURCES = {
    "forest": "forward-fly-above-snow-forest.mp4",
    "valley": "forward-fly-through-road-to-snow-valley.mp4",
}
FONTS = {
    "Cinzel": ("Cinzel.zip", "static/Cinzel-Regular.ttf"),
    "LobsterTwo": ("Lobster_Two.zip", "LobsterTwo-Regular.ttf"),
}


def fingerprint(path):
    with path.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    return {"name": path.name, "sha256": digest, "bytes": path.stat().st_size}


def probe(path):
    result = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
            str(path),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout)


def check_recording(path):
    if not path.is_file():
        raise FileNotFoundError(f"recording not found: {path}")
    try:
        info = probe(path)
        video = next(
            stream for stream in info["streams"] if stream["codec_type"] == "video"
        )
        duration = float(video.get("duration", info["format"].get("duration", 0)))
        if not isfinite(duration) or duration < 8:
            raise ValueError("recording must contain at least 8 seconds of video")
        return duration
    except (subprocess.CalledProcessError, StopIteration, KeyError, TypeError) as error:
        raise ValueError(f"cannot read recording: {path}") from error


def prepare_assets(assets, work, recording=None):
    paths = [assets / name for name in SOURCES.values()]
    paths += [assets / archive for archive, _ in FONTS.values()]
    for path in paths:
        if not path.is_file():
            raise FileNotFoundError(f"missing collection asset: {path}")
    recording_duration = None
    if recording is not None:
        recording_duration = check_recording(recording)
        paths.append(recording)
    ffmpeg = subprocess.run(
        ["ffmpeg", "-version"], capture_output=True, text=True, check=True
    ).stdout.splitlines()[0]
    settings = {
        "sources": [fingerprint(path) for path in paths],
        "ffmpeg": ffmpeg,
        "footage": {
            "size": [1920, 1080],
            "fps": 30,
            "seconds": 6.5,
            "crf": 18,
            "preset": "fast",
        },
        "recording": None
        if recording is None
        else {
            "size": [1600, 900],
            "fps": 30,
            "speed": recording_duration / 8,
            "seconds": 8,
        },
    }
    manifest_path = work / "assets.json"
    if manifest_path.exists():
        old = json.loads(manifest_path.read_text())
        if old["settings"] == settings and all(
            (work / path).is_file() and fingerprint(work / path) == identity
            for path, identity in old["prepared"].items()
        ):
            return old
    prepared = work / "prepared"
    prepared.mkdir(parents=True, exist_ok=True)
    for name, (archive, member) in FONTS.items():
        with zipfile.ZipFile(assets / archive) as bundle:
            (prepared / f"{name}.ttf").write_bytes(bundle.read(member))
            (prepared / f"{name}-OFL.txt").write_bytes(bundle.read("OFL.txt"))
    inputs = [
        (
            assets / name,
            key,
            6.5,
            "scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080",
        )
        for key, name in SOURCES.items()
    ]
    if recording is not None:
        inputs.append(
            (
                recording,
                "recording",
                8,
                f"setpts=(PTS-STARTPTS)*{8 / recording_duration:.17g},scale=1600:900:force_original_aspect_ratio=decrease,pad=1600:900:(ow-iw)/2:(oh-ih)/2",
            )
        )
    for source, name, duration, filters in inputs:
        temporary = prepared / f"{name}.partial.mp4"
        try:
            subprocess.run(
                [
                    "ffmpeg",
                    "-v",
                    "error",
                    "-y",
                    "-i",
                    str(source),
                    "-map",
                    "0:v:0",
                    "-vf",
                    filters + ",fps=30,setsar=1",
                    "-t",
                    str(duration),
                    "-an",
                    "-map_metadata",
                    "-1",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "fast",
                    "-crf",
                    "18",
                    "-pix_fmt",
                    "yuv420p",
                    "-threads",
                    "2",
                    str(temporary),
                ],
                check=True,
            )
            temporary.replace(prepared / f"{name}.mp4")
        finally:
            temporary.unlink(missing_ok=True)
    manifest = {
        "settings": settings,
        "prepared": {
            str(path.relative_to(work)): fingerprint(path)
            for path in sorted(prepared.iterdir())
            if path.is_file()
        },
    }
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def add_editor(project, recording):
    editor = project.root.group(start=2, duration=8, z=3)
    editor.opacity.keyframe(0, 0)
    editor.opacity.keyframe(0.5, 1)
    editor.opacity.keyframe(7.5, 1)
    editor.opacity.keyframe(8, 0)

    def text(value, position, size=28, fill="#fff2d6", font="Cinzel"):
        layer = editor.add(
            Text(value, font=f"prepared/{font}.ttf", font_size=size, fill=fill)
        )
        layer.transform.position = position
        return layer

    card = editor.add(
        Rectangle(width=1232, height=748, fill="#101c25", corner_radius=20)
    )
    card.transform.position = (0.63, 0.56)
    text("main.py  /  Neovim", (0.63, 0.245), 22, "#b4c9ce")
    for index, color in enumerate(("#df8578", "#dfb676", "#9db99a")):
        dot = editor.add(Circle(radius=6, fill=color))
        dot.transform.position = (0.328 + index * 0.012, 0.245)
    text("Write.\nRender.\nRepeat.", (0.166, 0.54), 84, font="LobsterTwo")
    text("PYTHON / VESTRA", (0.166, 0.79), 19, "#dfb676")
    if recording is None:
        text("A scene, written in Python.", (0.63, 0.46), 35)
        text("Your Neovim recording goes here", (0.63, 0.55), 22, "#b4c9ce")
        text("A whole take / eight seconds on screen", (0.63, 0.64), 18, "#dfb676")
        cursor = editor.add(Rectangle(width=13, height=27, fill="#dfb676"))
        cursor.transform.position = (0.63, 0.73)
        cursor.opacity.keyframe(0, 0.2)
        cursor.opacity.keyframe(4, 1)
        cursor.opacity.keyframe(8, 0.2)
    else:
        video = editor.add(Video(recording, sizing="fit"))
        video.transform.scale = 0.625
        video.transform.position = (0.63, 0.586)


def run(build_project):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets", type=Path, default=Path.home() / "drive/Assets")
    parser.add_argument(
        "--recording",
        type=Path,
        help="at least 8 seconds; the whole take is retimed into 8s",
    )
    parser.add_argument("--backend", choices=("cpu", "wgpu"), default="cpu")
    parser.add_argument(
        "--export-only",
        action="store_true",
        help="prepare assets and validate/export JSON",
    )
    args = parser.parse_args()
    work = ROOT / "target/code-to-video"
    prepare_assets(args.assets.expanduser().resolve(), work, args.recording)
    project = build_project(work, "prepared/recording.mp4" if args.recording else None)
    if not project.validate().is_valid:
        raise ValueError("example project failed semantic validation")
    snapshot = project.snapshot(output="render.mp4")
    snapshot.save(work / "project.json")
    if args.export_only:
        print(work / "project.json")
        return
    result = project.render(work / "render.mp4", backend=args.backend, overwrite=True)
    output = probe(work / "render.mp4")
    counted = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-count_frames",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "csv=p=0",
            str(work / "render.mp4"),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    video = next(
        stream for stream in output["streams"] if stream["codec_type"] == "video"
    )
    if (
        video["width"],
        video["height"],
        video["avg_frame_rate"],
        int(counted.stdout),
    ) != (1920, 1080, "30/1", 360):
        raise ValueError(
            "published output does not match 1920x1080 / 30fps / 360 decoded frames"
        )
    report = {
        "project": fingerprint(work / "project.json"),
        "output": fingerprint(work / "render.mp4"),
        "requested_backend": args.backend,
        "selected_backend": result.selected_backend,
        "encoder": result.encoder_backend,
        "elapsed_seconds": result.elapsed_seconds,
        "decoded_frames": int(counted.stdout),
        "probe": output,
    }
    (work / "render.json").write_text(json.dumps(report, indent=2) + "\n")
    print(work / "render.mp4")
