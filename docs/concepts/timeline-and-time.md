# Timeline and time

Vestra evaluates each composition in its own local time space. The project
frame rate converts that continuous timeline into output frames.

## Project and layer time

The project duration is the length of the root timeline when it is explicit.
The root composition uses that duration when a layer does not provide one.
Each layer has a local `start` and `duration`. A layer contributes only while
the parent composition's time lies in its interval.

For a media source, `source_start` chooses the source position at the layer's
start. `playback_rate` changes how quickly source time advances. These are
separate from the layer's placement time:

```python
layer = project.root.add(
    Video("clip.mp4"),
    start=2.0,
    duration=4.0,
    source_start=10.0,
    playback_rate=0.5,
)
```

This places four seconds of the video timeline starting at project time two.
It samples source time from ten seconds onward at half speed.

## Frames and frame rate

`fps` defines how Vestra maps output frame numbers to presentation time. A
fractional frame rate can be supplied as `(numerator, denominator)`. The
renderer works with exact frame-rate information internally, so avoid treating
`fps` as a rounded display number when matching an external edit.

Python properties and layer placement times are expressed in seconds. Native
frame APIs also accept frame numbers, nanoseconds, or seconds. Choose one unit
at an API boundary and convert once. Repeated floating-point conversions near
a boundary are a common source of off-by-one-frame mistakes.

## Nested time

A `CompositionLayer` has a parent-local start and duration. Its child layers
use child-local start and duration. A child layer at time `0.5` becomes visible
half a second after the group placement begins, subject to the group's active
interval. The child does not inherit the parent's absolute start as a new
authored value.

The same rule applies to transitions inside a child composition. A transition
between child siblings is placed in that child's local timeline.

## Audio time

Audio uses several related time domains. A clip's `start` is a project-timeline
position. `trim_start` and `trim_end` are positions in the source audio.
`fade_in` and `fade_out` are durations on the selected clip. Gain-automation
keyframes use clip-local time. Audio and visual evaluation meet on the project
timeline. Audio analysis signals are sampled while that timeline is evaluated;
they are control data, not a second playback clock.

## Boundaries

Use positive durations and non-negative starts. Keep a layer's source offset
inside the usable source when the source has finite media. Transitions and
fades occupy explicit intervals, so their endpoints must fit the layers or
clips they connect. Validation is the final authority for combinations that
cross media, frame, and output boundaries.
