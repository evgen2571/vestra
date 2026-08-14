from vestra import Project
from vestra.sources import Color

project = Project(size=(2, 2), fps=1, duration=1)
layer = project.root.add(Color("#ffffff"))
layer.opacity = "opaque"
layer.transform.position = (0.5, "bad")
layer.transform.anchor = (0.5, "bad")
layer.transform.scale = (1.0, "bad")
