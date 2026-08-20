# Effects, transitions, animation, flashes, and presets

These features act at different levels of the project. Keeping the levels
separate makes complex edits easier to reason about.

| Feature | What it changes |
| --- | --- |
| Animation | A property value over time |
| Effect | The appearance or processing of one layer or the project output |
| Transition | How two sibling layers exchange presentation over an interval |
| Flash | A short root-owned colour overlay |
| Preset | Reusable cinematic intent applied to an eligible layer |

Animation is the time-varying input. For example, keyframes can change opacity,
position, scale, rotation, or an effect parameter. An effect consumes those
values while processing a layer or output. A transition owns presentation
channels for its outgoing and incoming endpoints, so its animation is relative
to transition progress rather than a general layer property track.

Transitions belong to a composition and connect two layers in that same
composition. Composition layers can be endpoints, and a child composition can
have its own transitions between its children. A transition does not move
layers into a new composition.

Flashes are root overlays with their own start, duration, opacity, and fades.
They are useful for a beat or burst that should sit over the rendered project.
Presets are reusable values for supported image layers. A layer can have one
cinematic preset at a time. Presets do not replace ordinary transforms or
effects.

Before the first keyframe, a track evaluates to its base value. Between
keyframes it interpolates, and after the final keyframe it holds that final
value. The high-level API lowers all of these descriptors into the canonical
project before rendering. Their authored ownership stays distinct even when
the final frame combines them.
