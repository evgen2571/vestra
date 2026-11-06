"""Render the documentation homepage example. Run: python home_card.py"""

from vestra import Ellipse, Project, Text

project = Project(
    size=(960, 540),
    fps=30,
    duration=3,
    background="#172c4b",
)

project.root.add(
    Ellipse(width=152, height=152, fill="#ff947f"),
    position=(0.2, 0.5),
)
project.root.add(
    Text("Title card", font_size=56, fill="#f5f7fb"),
    position=(0.62, 0.5),
)

project.render("home-card.mp4", backend="cpu", overwrite=True)
