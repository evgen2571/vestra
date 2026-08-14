from vestra import Project
from vestra.sources import Color


project = Project(size=(2, 2), fps=1, duration=1)
layer = project.root.add(Color("#ffffff"))
signal = project.audio.signal.rms()
layer.opacity.bind(object())
layer.opacity.bind(signal, operation="invalid")
layer.transform.position.bind(signal)
layer.transform.position_x.value = 1
layer.transform.position_x.keyframe(0, 1)
