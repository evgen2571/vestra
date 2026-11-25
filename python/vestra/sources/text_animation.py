"""Clip-local word highlighting for a fully shaped text cue."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass

from ..authoring._internal import _number
from ..authoring.values import Color, color_to_canonical


@dataclass(frozen=True, slots=True)
class WordTiming:
    """Half-open clip-local seconds and Unicode character offsets."""

    start: float
    end: float
    text_start: int
    text_end: int

    def __post_init__(self) -> None:
        start = _number(self.start, "word start")
        end = _number(self.end, "word end")
        if not 0 <= start < end:
            raise ValueError("word times must satisfy 0 <= start < end")
        for offset in (self.text_start, self.text_end):
            if not isinstance(offset, int) or isinstance(offset, bool):
                raise TypeError("word text offsets must be integers")
        if not 0 <= self.text_start < self.text_end:
            raise ValueError("word text offsets must satisfy 0 <= text_start < text_end")
        object.__setattr__(self, "start", start)
        object.__setattr__(self, "end", end)

    def to_canonical(self) -> dict[str, object]:
        return {
            "start": self.start,
            "end": self.end,
            "text_start": self.text_start,
            "text_end": self.text_end,
        }


@dataclass(frozen=True, slots=True, init=False)
class WordHighlight:
    """An alternate fill and ordered, non-overlapping word timings."""

    color: str
    words: tuple[WordTiming, ...]

    def __init__(self, color: Color | str, words: Sequence[WordTiming]) -> None:
        timings = tuple(words)
        previous_end = 0.0
        for word in timings:
            if not isinstance(word, WordTiming):
                raise TypeError("words must contain WordTiming values")
            if word.start < previous_end:
                raise ValueError("word times must be ordered and must not overlap")
            previous_end = word.end
        object.__setattr__(self, "color", color_to_canonical(color))
        object.__setattr__(self, "words", timings)

    def to_canonical(self) -> dict[str, object]:
        return {"colour": self.color, "words": [word.to_canonical() for word in self.words]}
