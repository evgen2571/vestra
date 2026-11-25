---
description: Import a plain-text SRT, style caption clips and export a selectable MP4 subtitle track.
---

# Style and export subtitles

`Subtitles.from_srt("captions.srt")` reads numbered, plain-text UTF-8 cues.
Cue blocks use blank-line separators and `HH:MM:SS,mmm --> HH:MM:SS,mmm`
timing. Text can span multiple lines. Tags in the cue body are treated as text.
An invalid cue reports its block or cue number.

To write cues in Python, pass `Subtitles` tuples of `(start, end, text)` or
`SubtitleCue(start, end, text)` values. Times are finite, non-negative seconds
on the track's local timeline. A cue is active from its start up to its
exclusive end. Cues are sorted by start, adjacent cues are allowed, and
overlapping cues raise an error. Add separate groups if multiple captions
should appear simultaneously. An empty cue list has a duration of zero and
can be skipped.

## Place a burned-in track

`Subtitles.to_group()` places ordinary [Text](../../reference/sources/text.md)
clips in a [Group](nested-compositions.md), using the same font, fill,
alignment, wrapping width, outline and background for each cue. The default
position is `(0.5, 0.9)` of the local canvas. Each cue uses a bottom-center
anchor so multiline captions extend upward. Set `max_width` in pixels to wrap
within a region, and use `position` to move the track. With no width, text
keeps its natural line widths.

Place the result with `project.root.add(captions.to_group(max_width=520),
duration=captions.duration)`. Give the placement a `start` to offset all cues,
or place it in a nested composition to use that composition's local time.
Apply whole-cue keyframes to each returned group's child placements before
adding the group to the project.

## Run the subtitle example

The standalone script creates a sample SRT, sets explicit character ranges and
cue-local timing for a repeated-word highlight, and animates a separate title's
reveal. It uses the bundled font and generates its own backdrop.
[Download the example](https://raw.githubusercontent.com/evgen2571/vestra/develop/examples/tutorials/styled_subtitles.py)

```python
--8<-- "examples/tutorials/styled_subtitles.py"
```

Run it from the repository root. It writes `styled-subtitles.mp4` to the
current directory and replaces that file on rerun.

```sh
python examples/tutorials/styled_subtitles.py
```

The example includes the burned-in group and passes `subtitles=subtitles.cues`
to opt into a [selectable MP4 track](../../reference/render-outputs.md).
Players that automatically enable the selectable track can show both sets of
captions. Turn it off in the player to view the burned-in styles by themselves.
The selectable track uses project-local seconds. Offset its cues explicitly
when the burned-in group is placed at a different start.
