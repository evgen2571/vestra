# Transitions

Transitions are composition-local placements connecting two sibling endpoints.
Each placement has `id`, `outgoing`, `incoming`, `start`, `duration`, and a
`definition` with outgoing and incoming presentation channels.

Presentation channels can contain normalized tracks for `opacity`,
`position_offset`, `scale_multiplier`, `rotation_offset_degrees`, and attached
effects. Progress is in the inclusive range `0..=1`. The transition interval is
in the owning composition's local timeline. Endpoints must be owned by that
composition, and the interval must fit the endpoint timing rules.

Python convenience constructors include `Crossfade`, `DirectionalPush` with
`PushLeft`, `PushRight`, `PushUp`, and `PushDown`, `ZoomCrossfade` with
`ZoomIn` and `ZoomOut`, `BlurCrossfade`, `ZoomBlurTransition`, `WhipPanLeft`,
`WhipPanRight`, and `CustomTransition`. `Animate` supplies normalized tracks.
The canonical representation is generic `TransitionPlacement`; historical
transition representation names are not part of the current contract.

Validation checks endpoint ownership, positive duration, overlap and fitting,
ordered normalized keyframes, and effect validity. Nested compositions may own
their own transitions. CPU and WGPU consume the same canonical presentation
model; renderer-specific limitations remain subject to current tests.
