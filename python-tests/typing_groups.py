from vestra import Project, Transform
from vestra.sources import Circle, Group, GroupChild, Image, Rectangle, Text, Video

project = Project(size=(32, 32), fps=2, duration=5)
transform = Transform()
transform.position.keyframe(0, (0.25, 0.5))
group = Group(position=(0.75, 0.5), rotation=15)
child: GroupChild = group.add(Rectangle(width=8, height=8, fill="#ff0000", scale=2),
                              position=(0.4, 0.5), rotation_degrees=30)
child.transform.rotation.keyframe(0, 0)
group.add(Group([Circle(radius=4, fill="#ffffff", anchor=(0.5, 0.5))]))
group.add(Image("frame.png", transform=transform))
group.add(Video("clip.mp4", rotation_degrees=15), duration=1)
group.add(Text("V", font="font.ttf", font_size=8, position=(0.5, 0.5)))
project.root.add(group, position=(0.25, 0.5))
project.root.group().add(group, scale=0.5)
project.root.add(group, transform=transform)
