from vestra import Project
from vestra.effects import Brightness, CameraShake, Vignette
from vestra.sources import Color

project = Project(size=(2, 2), fps=1, duration=1)
layer = project.root.add(Color("#ffffff"))
brightness = layer.effects.add(Brightness(0))
brightness.amount.bind(object())
vignette = layer.effects.add(Vignette(0.5, 1, 0.5, "#000000"))
vignette.softness.bind(project.audio.signal.rms())
project.post_effects.add(CameraShake(1, 1, 0.1, 1, 1, 0, 1))
