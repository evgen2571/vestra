from vestra import Project, Transform
from vestra.sources import Circle, Group, GroupChild, Image, Rectangle, Text, Video

project = Project(size=(32, 32), fps=2, duration=5)
transform = Transform()
transform.position.keyframe(0, (0.25, 0.5))
group = Group(size=(600, 300.5), position=(0.75, 0.5), rotation=15)
group_size: tuple[float, float] | None = group.size
child: GroupChild = group.add(Rectangle(width=8, height=8, fill="#ff0000", scale=2),
                              position=(0.4, 0.5), rotation_degrees=30)
child.transform.rotation.keyframe(0, 0)
group.add(Group([Circle(radius=4, fill="#ffffff", anchor=(0.5, 0.5))], size=(60, 30)))
group.add(Image("frame.png", transform=transform))
group.add(Video("clip.mp4", rotation_degrees=15), duration=1)
group.add(Text("V", font="font.ttf", font_size=8, position=(0.5, 0.5)))
project.root.add(group, position=(0.25, 0.5))
project.root.group().add(group, scale=0.5)
project.root.add(group, transform=transform)

from vestra import FrameRate
from vestra.authoring import ProjectBuilder

builder = ProjectBuilder(width=32, height=32, frame_rate=FrameRate(2, 1), output_path="out.mp4", duration=5)
advanced = builder.add_group_clip(clips=[], size=(600.5, 300), start=0, duration=5, layer=0)
advanced_size: tuple[float, float] | None = advanced.size
