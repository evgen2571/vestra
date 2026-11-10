"""Run with vestra[numpy] installed: python frame_exports.py."""

from pathlib import Path

from vestra import Project, Rectangle

output = Path("frame-exports")
output.mkdir(exist_ok=True)

project = Project(size=(320, 180), fps=24, duration=2, background="#102030")
bar = project.root.add(Rectangle(width=160, height=16, fill="#f59e69"))
bar.opacity.keyframe(0, 0)
bar.opacity.keyframe(1, 1)

frame = project.render_frame(0.5, output / "poster.png", overwrite=True)
frame.save(output / "poster.jpg", overwrite=True)
frame.save(output / "poster.webp", overwrite=True)

project.render_frames(
    times=[0, 0.5, 1], directory=output / "selected", format="webp", overwrite=True
)
count = project.render_frames(directory=output / "sequence", end=1, overwrite=True)
print(f"Saved {count} sequence frames")

pixels = project.get_frame(frame_number=12)
print(pixels.shape, pixels.dtype, pixels.strides)

result = project.render(output / "clip.mp4", start=0.5, end=1, overwrite=True)
print(f"Saved {result.total_frames} selected video frames")

project.render_preview(output / "preview.mp4", overwrite=True)
project.render(output / "small.mp4", resolution=(160, 90), fps=12, overwrite=True)
project.render(output / "animation.gif", fps=12, end=1, overwrite=True)
project.render(output / "animation.webp", fps=12, end=1, overwrite=True)
