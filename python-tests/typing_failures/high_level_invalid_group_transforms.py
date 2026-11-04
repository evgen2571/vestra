from vestra import Project
from vestra.sources import Group, Rectangle

project = Project(size=(32, 32), fps=2, duration=5)
Group(position="center")
Rectangle(width=8, height=8, fill="#fff", rotation="left")
project.root.add(Group(), scale="big")
Group().add(Group(), anchor="center")
