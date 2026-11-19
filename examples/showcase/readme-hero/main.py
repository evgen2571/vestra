from vestra import Interpolation, Project, Rectangle

project = Project(
    size=(960, 540),
    fps=30,
    duration=2,
    background="#101c24",
)
track = Rectangle(width=640, height=6, fill="#36545b")
project.root.add(track)

tile = project.root.add(
    Rectangle(
        width=200,
        height=120,
        corner_radius=20,
        fill="#b8e3bc",
    )
)

move = tile.transform.position
ease = Interpolation.EASE_IN_OUT
move.keyframe(0, (0.25, 0.5))
move.keyframe(1, (0.75, 0.5), interpolation=ease)

project.render("out.mp4", backend="cpu")
