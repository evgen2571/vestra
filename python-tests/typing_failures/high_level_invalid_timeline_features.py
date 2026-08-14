from vestra import Flash, Project
from vestra.sources import Color, Image

project = Project(size=(8, 6), fps=10, duration=2)
first = project.root.add(Image("first.png"))
second = project.root.add(Image("second.png"))
project.root.transitions.add(first, second, object())
project.flashes.add(object())
Flash("bad", 1, "#ffffff")
project.root.add(Color("#ffffff")).presets.add(object())
