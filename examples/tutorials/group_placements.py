"""Place one completed Group twice; run to replace group-placements.mp4."""

from vestra import Circle, Group, Project, Rectangle, Text

project = Project(
    size=(640, 360),
    fps=30,
    duration=3,
    background="#101c27",
    output_audio=False,
)

card = Group(size=(320, 160))

card.add(Rectangle(width=320, height=160, fill="#dbe4e8", corner_radius=12))

card.add(Circle(radius=24, fill="#659bba"), position=(0.18, 0.5))

card.add(Text("Field notes", font_size=30, fill="#1b3445"), position=(0.59, 0.44))
card.add(Rectangle(width=140, height=4, fill="#659bba"), position=(0.59, 0.67))

left = project.root.add(card, position=(0.25, 0.5), scale=0.8)
right = project.root.add(card, position=(0.75, 0.5), scale=0.8)

right.transform.rotation = 8
right.opacity = 0.55

project.root.add(Text("First placement", font_size=24), position=(0.25, 0.22))
project.root.add(Text("Second placement", font_size=24), position=(0.75, 0.22))

project.root.add(Text("Scale 0.8", font_size=22), position=(0.25, 0.80))
project.root.add(Text("Rotate 8° · opacity 0.55", font_size=22), position=(0.75, 0.80))

if __name__ == "__main__":
    project.render("group-placements.mp4", backend="cpu", overwrite=True)
