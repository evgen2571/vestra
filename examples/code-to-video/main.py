"""A twelve-second scene whose editor recording becomes part of the scene."""

from vestra import Project
from vestra.effects import Saturation
from vestra.sources import Color, Rectangle, Text, Video

from support import add_editor, run


def build_project(work, recording=None):
    project = Project(size=(1920, 1080), fps=30, duration=12, base_directory=work)
    for name, start in (("forest", 0), ("valley", 5.5)):
        shot = project.root.add(
            Video(f"prepared/{name}.mp4", sizing="cover"), start=start, duration=6.5
        )
        shot.opacity.keyframe(0, 0)
        shot.opacity.keyframe(0.75, 1)
        shot.effects.add(Saturation(0.65))
        shot.transform.scale.keyframe(0, 1)
        shot.transform.scale.keyframe(6.5, 1.04)
    project.root.add(Color("#07151c"), opacity=0.42, z=1)
    title = project.root.add(
        Text("Code to video", font="prepared/Cinzel.ttf", font_size=88, fill="#fff2d6"),
        z=2,
    )
    title.transform.position = (0.5, 0.13)
    title.opacity.keyframe(0, 0)
    title.opacity.keyframe(1, 1)
    add_editor(project, recording)
    line = project.root.add(Rectangle(width=1740, height=4, fill="#dfb676"), z=4)
    line.transform.anchor = (0, 0.5)
    line.transform.position = (0.047, 0.96)
    line.transform.scale.keyframe(0, (0.001, 1))
    line.transform.scale.keyframe(12, (1, 1))
    return project


if __name__ == "__main__":
    run(build_project)
