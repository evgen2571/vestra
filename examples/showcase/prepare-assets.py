#!/usr/bin/env python3
"""Reproduce the downloaded footage and original audio used by the showcases."""

import argparse
import hashlib
import math
from pathlib import Path
import shutil
import struct
import subprocess
import urllib.request
import wave

ROOT = Path(__file__).resolve().parent
CITY_URL = (
    "https://upload.wikimedia.org/wikipedia/commons/d/d7/City_timelapse_video.webm"
)
CITY_SHA256 = "42ebfa7a64b4538497dd08891b1aa11f20545940948f06473a0e77a6d917a450"


def download(url: str, digest: str, target: Path) -> None:
    if target.is_file() and hashlib.sha256(target.read_bytes()).hexdigest() == digest:
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    partial = target.with_suffix(target.suffix + ".partial")
    try:
        request = urllib.request.Request(
            url, headers={"User-Agent": "Vestra-showcase/0.1"}
        )
        with (
            urllib.request.urlopen(request, timeout=60) as response,
            partial.open("wb") as output,
        ):
            shutil.copyfileobj(response, output)
        if hashlib.sha256(partial.read_bytes()).hexdigest() != digest:
            raise ValueError(f"checksum mismatch for {target.name}")
        partial.replace(target)
    finally:
        partial.unlink(missing_ok=True)


def write_audio(path: Path) -> None:
    """An original sixteen-second synth loop, with a soft pulse and evolving chords."""
    rate, duration = 48000, 16
    chords = (
        (130.81, 164.81, 196.00),
        (110.00, 130.81, 164.81),
        (87.31, 110.00, 130.81),
        (98.00, 123.47, 146.83),
    )
    samples = bytearray()
    for index in range(rate * duration):
        time = index / rate
        beat = time % 0.5
        chord = chords[min(int(time / 4), 3)]
        # Soft harmonics, a short percussive bass pulse, and a quiet arpeggio.
        pad = sum(math.sin(math.tau * hz * time) for hz in chord) / 3
        pulse = math.sin(math.tau * 55 * time) * math.exp(-beat * 18)
        note = chord[int(time * 4) % 3] * 4
        pluck = math.sin(math.tau * note * time) * math.exp(-(time % 0.25) * 16)
        fade = min(1, time / 0.4, (duration - time) / 0.8)
        value = fade * (0.16 * pad + 0.20 * pulse + 0.05 * pluck)
        samples.extend(struct.pack("<h", round(value * 32767)))
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(rate)
        output.writeframes(samples)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--audio-only",
        action="store_true",
        help="generate audio without downloading footage",
    )
    args = parser.parse_args()
    print(
        "Original synth audio: CC0. Footage: City timelapse video / Location Kenya / CC BY 3.0."
    )
    print(
        "Read examples/showcase/ASSETS.md for source, license and redistribution requirements."
    )
    write_audio(ROOT / "assets/synth.wav")
    if args.audio_only:
        return
    source = ROOT.parents[1] / "target/showcase-sources/city.webm"
    download(CITY_URL, CITY_SHA256, source)
    for number, start in enumerate((3, 34, 59), 1):
        destination = ROOT / f"assets/city-{number}.mp4"
        subprocess.run(
            [
                "ffmpeg",
                "-v",
                "error",
                "-ss",
                str(start),
                "-i",
                str(source),
                "-t",
                "5",
                "-an",
                "-vf",
                "setpts=PTS-STARTPTS,scale=1280:720,fps=30",
                "-map_metadata",
                "-1",
                "-c:v",
                "libx264",
                "-crf",
                "18",
                "-pix_fmt",
                "yuv420p",
                "-y",
                str(destination),
            ],
            check=True,
        )
        print(f"Prepared {destination.relative_to(ROOT)}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"Showcase asset preparation failed: {error}") from error
