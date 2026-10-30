"""Deterministic moving inputs and sparse validation of long stream-copy sources."""

import hashlib
import json
import math
import subprocess

from benchmarks.comparisons.moviepy.comparison import (
    digest,
    validate_output,
    version,
    write_json,
)


def command(args, timeout=300):
    result = subprocess.run(
        [str(a) for a in args], capture_output=True, timeout=timeout
    )
    if result.returncode:
        raise ValueError(
            f"command failed ({result.returncode}): {args}\n{result.stderr.decode(errors='replace')[-4000:]}"
        )
    return result.stdout


def portable_inputs(directory, *, width=640, height=360):
    directory.mkdir(parents=True, exist_ok=False)
    settings = dict(width=width, height=height, fps=30, duration=12)
    manifest = dict(
        schema_version=1,
        generator_version=1,
        ffmpeg=version(["ffmpeg", "-version"]),
        settings=settings,
        sources={},
        prepared={},
    )
    # Supersample moving bars, discs and counters to avoid single-pixel aliasing
    # dominating the comparison of different resamplers. Motion/detail remain;
    # separate hue/flip transformations keep overlays distinct from the base.
    for key, effect in {
        "forest": "hue=h=0",
        "valley": "hue=h=105,hflip",
        "tear": "hue=h=230,vflip",
    }.items():
        output = directory / f"{key}.mp4"
        video_filter = f"{effect},scale={width}:{height}:flags=bicubic,gblur=sigma=1"
        args = [
            "ffmpeg",
            "-v",
            "error",
            "-nostdin",
            "-n",
            "-f",
            "lavfi",
            "-i",
            f"testsrc2=size={width * 2}x{height * 2}:rate=30:duration=12",
            "-vf",
            video_filter,
            "-an",
            "-frames:v",
            "360",
            "-c:v",
            "libx264",
            "-crf",
            "18",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "2",
            "-g",
            "60",
            "-sc_threshold",
            "0",
            "-map_metadata",
            "-1",
            "-fflags",
            "+bitexact",
            "-flags:v",
            "+bitexact",
            output,
        ]
        command(args)
        manifest["sources"][key] = dict(
            generator="FFmpeg testsrc2", filter=video_filter, supersampling=2
        )
        manifest["prepared"][key] = dict(
            filename=output.name,
            sha256=digest(output),
            validation=validate_output(output, settings),
            command=["<inputs>/" + output.name if a == output else a for a in args],
        )
    write_json(directory / "manifest.json", manifest)
    return manifest


def frame_at(path, seconds, width, height):
    data = command(
        [
            "ffmpeg",
            "-v",
            "error",
            "-xerror",
            "-nostdin",
            "-ss",
            f"{seconds:.9f}",
            "-i",
            path,
            "-frames:v",
            "1",
            "-threads",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]
    )
    if len(data) != width * height * 3:
        raise ValueError(f"missing frame at {seconds} seconds")
    return data


def long_source(segment, output, *, duration=7200, width=640, height=360):
    """Remux repeated 12s GOPs, then validate every presentation timestamp.

    ffprobe streams one timestamp per frame to disk; Python checks it linearly
    without retaining a duration-sized list. Sparse decodes prove seek coverage.
    """
    required = segment.stat().st_size * math.ceil(duration / 12) + 128 * 2**20
    import shutil

    if shutil.disk_usage(output.parent).free < required + 2**30:
        raise ValueError(
            f"long source needs approximately {required / 2**30:.2f} GiB plus 1 GiB reserve"
        )
    args = [
        "ffmpeg",
        "-v",
        "error",
        "-nostdin",
        "-n",
        "-stream_loop",
        "-1",
        "-i",
        segment,
        "-frames:v",
        str(int(duration * 30)),
        "-map",
        "0:v:0",
        "-an",
        "-c",
        "copy",
        "-map_metadata",
        "-1",
        output,
    ]
    command(args, timeout=600)
    metadata = json.loads(
        command(
            [
                "ffprobe",
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
                output,
            ]
        )
    )
    video = next(s for s in metadata["streams"] if s["codec_type"] == "video")
    expected = duration * 30
    if (
        abs(float(video["duration"]) - duration) > 1 / 30
        or int(video["nb_frames"]) != expected
    ):
        raise ValueError("long-source duration/frame count mismatch")
    timestamps = output.with_suffix(".timestamps.local")
    try:
        with timestamps.open("w", encoding="utf-8") as stream:
            decoded = subprocess.run(
                [
                    "ffprobe",
                    "-v",
                    "error",
                    "-select_streams",
                    "v:0",
                    "-show_frames",
                    "-show_entries",
                    "frame=best_effort_timestamp_time",
                    "-of",
                    "csv=p=0",
                    str(output),
                ],
                check=True,
                stdout=stream,
                stderr=subprocess.PIPE,
                timeout=900,
            )
            if decoded.stderr.strip():
                raise ValueError(
                    f"long-source decode diagnostics: {decoded.stderr.decode(errors='replace')[-4000:]}"
                )
        count = 0
        with timestamps.open(encoding="utf-8") as stream:
            for line in stream:
                token = line.strip().split(",")[0]
                if not token:
                    continue
                pts = float(token)
                if abs(pts - count / 30) > 0.00001:
                    raise ValueError(
                        f"long-source timestamp discontinuity at frame {count}: {pts}"
                    )
                count += 1
        if count != expected:
            raise ValueError("long-source decoded timestamp count mismatch")
    finally:
        timestamps.unlink(missing_ok=True)
    coverage = []
    for frame in [
        0,
        1,
        359,
        360,
        expected // 2,
        expected // 2 + 1,
        expected - 2,
        expected - 1,
    ]:
        seconds = frame / 30
        data = frame_at(output, seconds, width, height)
        reference = frame_at(segment, (frame % 360) / 30, width, height)
        if data != reference:
            raise ValueError(
                f"long-source seek pixels differ from segment at frame {frame}"
            )
        coverage.append(
            dict(
                frame=frame,
                seconds=seconds,
                rgb_sha256=hashlib.sha256(data).hexdigest(),
                matches_segment=True,
            )
        )
    result = dict(
        status="passed",
        filename=output.name,
        duration=duration,
        frames=expected,
        width=width,
        height=height,
        fps=30,
        sha256=digest(output),
        segment_sha256=digest(segment),
        generation="12s moving H264 segment repeated with stream copy",
        command=[
            "<segment>" if a == segment else "<long-source>" if a == output else a
            for a in args
        ],
        timestamp_count=count,
        continuous_cfr_pts=True,
        coverage=coverage,
        full_timestamp_decode=True,
        sparse_pixel_validation=True,
    )
    write_json(output.with_suffix(".fixture.json"), result)
    return result
