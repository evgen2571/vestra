from vestra import Flash, Project
from vestra.effects import Brightness
from vestra.presets import Preset
from vestra.sources import Image
from vestra.transitions import (
    Crossfade,
    DirectionalPush,
    PushLeft,
    ZoomIn,
    ZoomCrossfade,
)

project = Project(size=(8, 6), fps=10, duration=2)
outgoing = project.root.add(Image("first.png"))
incoming = project.root.add(Image("second.png"))
project.root.transitions.add(outgoing, incoming, Crossfade(), start=0, duration=1)
project.root.transitions.add(
    outgoing, incoming, ZoomCrossfade(outgoing_zoom=1.2, incoming_start_zoom=0.8), start=0, duration=1
)
project.root.transitions.add(
    outgoing, incoming, PushLeft(), start=0, duration=1
)
project.root.transitions.add(
    outgoing,
    incoming,
    DirectionalPush(angle_degrees=90, distance=1), start=0, duration=1
)
project.root.transitions.add(
    outgoing, incoming, ZoomIn(), start=0, duration=1
)
outgoing.presets.add(Preset("impact", seed=7))
outgoing.effects.add(Brightness(0.1))
flash: Flash = project.flashes.add(Flash(0.5, 0.2, "#ffffff"))
assert flash.id is not None
