from vestra import Flash, Project
from vestra.effects import Brightness
from vestra.presets import Preset
from vestra.sources import Image
from vestra.transitions import (
    Crossfade,
    DirectionalPush,
    FlashCut,
    ZoomBlur,
    ZoomCrossfade,
)

project = Project(size=(8, 6), fps=10, duration=2)
outgoing = project.root.add(Image("first.png"))
incoming = project.root.add(Image("second.png"))
project.root.transitions.add(outgoing, incoming, Crossfade(0, 1))
project.root.transitions.add(
    outgoing, incoming, ZoomCrossfade(0, 1, outgoing_zoom=1.2, incoming_start_zoom=0.8)
)
project.root.transitions.add(
    outgoing, incoming, FlashCut(0, 1, colour="#ffffff", intensity=0.5)
)
project.root.transitions.add(
    outgoing,
    incoming,
    DirectionalPush(0, 1, angle_degrees=90, distance=1, blur_radius=2),
)
project.root.transitions.add(
    outgoing,
    incoming,
    ZoomBlur(0, 1, outgoing_zoom=1.2, incoming_start_zoom=0.8, blur_radius=2),
)
outgoing.presets.add(Preset("impact", seed=7))
outgoing.effects.add(Brightness(0.1))
flash: Flash = project.flashes.add(Flash(0.5, 0.2, "#ffffff"))
assert flash.id is not None
