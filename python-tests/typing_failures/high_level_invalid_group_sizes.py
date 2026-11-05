from vestra import FrameRate
from vestra.authoring import ProjectBuilder
from vestra.sources import Group

Group(size="600x300")
Group(size=(600, "300"))
Group(size=(600,))
Group().size = (600, 300)
builder = ProjectBuilder(width=32, height=32, frame_rate=FrameRate(2, 1), output_path="out.mp4", duration=5)
builder.add_group_clip(clips=[], size=(600, "300"), start=0, duration=5, layer=0)
