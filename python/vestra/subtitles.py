"""Plain-text subtitle cues and burned-in caption authoring."""

from __future__ import annotations

import os
import re
from collections.abc import Iterable, Sequence
from itertools import groupby, pairwise
from math import modf
from pathlib import Path
from typing import NamedTuple, Self

from .authoring._internal import _number
from .authoring.values import Color, Point
from .sources import Group, GroupChild, Text
from .sources.text import TextAlignment

_NANOS_PER_SECOND = 1_000_000_000
_TIMESTAMP = r"([0-9]{2,}):([0-5][0-9]):([0-5][0-9]),([0-9]{3})"
_SRT_TIMING = re.compile(rf"{_TIMESTAMP}[ \t]*-->[ \t]*{_TIMESTAMP}")


class SubtitleCue(NamedTuple):
    """A plain-text cue with start and exclusive end in track-local seconds."""

    start: float
    end: float
    text: str


def _cue_time(value: float, name: str) -> tuple[float, int]:
    try:
        seconds = _number(value, name)
    except OverflowError as error:
        raise ValueError(f"{name} must be finite") from error
    nanos = seconds * _NANOS_PER_SECOND
    if not 0 <= nanos < 2**64:
        raise ValueError(
            f"{name} must be non-negative and within the supported timeline"
        )
    fraction, whole = modf(nanos)
    return seconds, int(whole) + int(fraction >= 0.5)


def _srt_seconds(fields: tuple[str, ...]) -> float:
    hours, minutes, seconds, millis = map(int, fields)
    return (((hours * 60 + minutes) * 60 + seconds) * 1000 + millis) / 1000


class Subtitles:
    """Ordered, nonoverlapping captions that can be placed as a Group."""

    __slots__ = ("_cues", "_durations")

    def __init__(self, cues: Iterable[tuple[float, float, str]]) -> None:
        entries = []
        for index, cue in enumerate(cues, 1):
            context = f"subtitle cue {index}"
            if not isinstance(cue, tuple) or len(cue) != 3:
                raise TypeError(f"{context} must be a (start, end, text) tuple")
            start, end, text = cue
            start, start_ns = _cue_time(start, f"{context} start")
            end, end_ns = _cue_time(end, f"{context} end")
            if end_ns <= start_ns:
                raise ValueError(
                    f"{context} end must follow start on the nanosecond timeline"
                )
            if not isinstance(text, str):
                raise TypeError(f"{context} text must be a string")
            if not text.strip():
                raise ValueError(f"{context} text must not be blank")
            entries.append((start_ns, end_ns, index, SubtitleCue(start, end, text)))

        entries.sort(key=lambda entry: entry[0])
        for previous, current in pairwise(entries):
            if current[0] < previous[1]:
                raise ValueError(
                    f"subtitle cue {current[2]} overlaps cue {previous[2]}"
                )

        self._cues = tuple(cue for _, _, _, cue in entries)
        # Quantize the endpoints before subtraction so adjacent cues share a boundary.
        self._durations = tuple(
            (end - start) / _NANOS_PER_SECOND for start, end, _, _ in entries
        )

    @property
    def cues(self) -> tuple[SubtitleCue, ...]:
        """Validated cues in chronological order."""
        return self._cues

    @property
    def duration(self) -> float:
        """The final cue's end in seconds, or zero for an empty track."""
        return self._cues[-1].end if self._cues else 0.0

    @classmethod
    def from_srt(cls, path: str | os.PathLike[str]) -> Self:
        """Read numbered, plain-text SRT cues from a UTF-8 file."""
        lines = Path(path).read_text(encoding="utf-8-sig").split("\n")
        cues = []
        blocks = (
            list(block)
            for present, block in groupby(lines, key=lambda line: bool(line.strip()))
            if present
        )
        for index, block in enumerate(blocks, 1):
            number = block[0].strip()
            if not number.isascii() or not number.isdecimal():
                raise ValueError(
                    f"SRT cue block {index} must begin with a numeric identifier"
                )
            timing = _SRT_TIMING.fullmatch(block[1].strip()) if len(block) > 1 else None
            if timing is None:
                raise ValueError(
                    f"SRT cue {number} must have HH:MM:SS,mmm --> HH:MM:SS,mmm timing"
                )
            if len(block) < 3:
                raise ValueError(f"SRT cue {number} must contain text")
            fields = timing.groups()
            try:
                start = _srt_seconds(fields[:4])
                end = _srt_seconds(fields[4:])
            except (OverflowError, ValueError) as error:
                raise ValueError(
                    f"SRT cue {number} has an invalid timestamp"
                ) from error
            if end <= start:
                raise ValueError(f"SRT cue {number} end must follow start")
            cues.append(SubtitleCue(start, end, "\n".join(block[2:])))
        return cls(cues)

    def to_group(
        self,
        *,
        position: Point | tuple[float, float] = (0.5, 0.9),
        max_width: float | None = None,
        font: str | os.PathLike[str] | None = None,
        fallback_fonts: Sequence[str | os.PathLike[str]] = (),
        font_size: float = 48,
        fill: Color | str = "#ffffff",
        stroke_color: Color | str | None = None,
        stroke_width: float = 0.0,
        background_color: Color | str | None = None,
        background_padding: float = 0.0,
        align: TextAlignment = "center",
        line_spacing: float = 1.0,
        letter_spacing: float = 0.0,
    ) -> Group:
        """Create a Group with Text clips sharing one style and bottom-center placement."""
        return Group(
            GroupChild(
                Text(
                    cue.text,
                    font=font,
                    fallback_fonts=fallback_fonts,
                    font_size=font_size,
                    fill=fill,
                    stroke_color=stroke_color,
                    stroke_width=stroke_width,
                    background_color=background_color,
                    background_padding=background_padding,
                    align=align,
                    max_width=max_width,
                    line_spacing=line_spacing,
                    letter_spacing=letter_spacing,
                ),
                start=cue.start,
                duration=duration,
                position=position,
                anchor=(0.5, 1.0),
            )
            for cue, duration in zip(self._cues, self._durations, strict=True)
        )


__all__ = ["SubtitleCue", "Subtitles"]
