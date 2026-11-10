#!/usr/bin/env python3
"""Verify shared footage and reproduce the original showcase audio."""

import argparse
import math
from pathlib import Path
import struct
import sys
import wave

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parents[1]))

from benchmarks.comparisons.moviepy.comparison import load_inputs  # noqa: E402


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
        help="generate audio without checking bundled footage",
    )
    args = parser.parse_args()
    write_audio(ROOT / "assets/synth.wav")
    if args.audio_only:
        return
    load_inputs(ROOT.parents[1] / "assets/footage")
    print("Verified bundled footage; generated synth audio.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        raise SystemExit(f"Showcase asset preparation failed: {error}") from error
