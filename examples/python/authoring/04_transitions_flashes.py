"""Create an explicitly fitted crossfade and a standalone flash.

Run from the repository root: uv run python examples/python/authoring/04_transitions_flashes.py
"""

from vestra import Crossfade, FrameRate
from vestra.authoring import ProjectBuilder

builder = ProjectBuilder(
    width=320, height=180, frame_rate=FrameRate(30, 1), output_path="transitions.mp4"
)

photo = builder.add_image_asset("assets/fixtures/red.png")
outgoing = builder.add_image_clip(source=photo, start=0, duration=1, layer=0)
incoming = builder.add_image_clip(source=photo, start=0.75, duration=1, layer=1)
# The clips overlap from 0.75 to 1 second; fit the transition in that interval.
builder.transitions.add_transition(
    outgoing=outgoing,
    incoming=incoming,
    definition=Crossfade().to_canonical(),
    start=0.75,
    duration=0.25,
)
builder.flashes.add(start=0.8, duration=0.1, colour="#ffffff", opacity=0.4, layer=2)
