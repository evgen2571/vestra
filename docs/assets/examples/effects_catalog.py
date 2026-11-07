"""Render source/treatment comparisons with a photograph and procedural scenes.

Keep landscape.jpg beside this file. Source and license: learning-inputs.txt.

Run with --effect gaussian-blur (or any key in effect_cases()). Outputs are
written to the current directory; rerunning replaces the named output.
"""

import argparse
from pathlib import Path
import subprocess

from vestra import Circle, Color, Group, Image, Project, Rectangle, Text
from vestra.effects import (
    Ascii,
    Bloom,
    Brightness,
    CameraShake,
    ChromaticAberration,
    ColorAdjust,
    Contrast,
    Crt,
    DirectionalBlur,
    GaussianBlur,
    Glow,
    Halftone,
    MotionBlur,
    MotionTile,
    OrderedDither,
    PaletteMap,
    PixelSort,
    PseudoAscii,
    RadialBlur,
    Saturation,
    Sharpen,
    Tint,
    Vignette,
    ZoomBlur,
)

SIZE = (640, 520)
SCENE = (640, 220)
DURATION = 3
PHOTO = Path(__file__).with_name("landscape.jpg")
PALETTE = ("#101b32", "#476b80", "#d6a46b", "#fff2cf")


def effect_cases():
    """Each preview changes one effect; values are deliberately visible at 640 px."""
    return {
        "original": None,
        "brightness": Brightness(amount=0.15),
        "contrast": Contrast(amount=1.5),
        "saturation": Saturation(amount=0.1),
        "tint": Tint(colour="#4ed5bf", amount=0.45),
        "color-adjust": ColorAdjust(
            exposure=0.5, gamma=1.3, black_point=0, white_point=1
        ),
        "gaussian-blur": GaussianBlur(radius=5),
        "directional-blur": DirectionalBlur(radius=16, angle_degrees=25),
        "zoom-blur": ZoomBlur(radius=18, samples=24, anchor=(0.5, 0.5)),
        "radial-blur": RadialBlur(amount=12, center=(0.5, 0.5)),
        "glow": Glow(threshold=0.55, radius=14, intensity=1.8, colour="#8adfff"),
        "bloom": Bloom(threshold=0.55, radius=14, intensity=1.8),
        "chromatic-aberration": ChromaticAberration(amount=6, angle_degrees=0),
        "vignette": Vignette(amount=0.8, radius=0.5, softness=0.4, colour="#000000"),
        "sharpen": Sharpen(amount=2.5, radius=3),
        "palette-map": PaletteMap(palette=PALETTE, mode="nearest"),
        "ordered-dither": OrderedDither(palette=PALETTE, matrix="bayer8", scale=1),
        "blue-noise": OrderedDither(palette=PALETTE, matrix="blue_noise", seed=37),
        "halftone": Halftone(mode="source", cell_size=5, softness=0.5),
        "pixel-sort": PixelSort(direction="horizontal", segment_length=96, amount=1),
        "ascii": Ascii(
            cell_width=10, cell_height=14, foreground="#efe1b8", background="#172126"
        ),
        "pseudo-ascii": PseudoAscii(
            cell_width=10, cell_height=14, foreground="#efe1b8", background="#172126"
        ),
        "crt": Crt(period=2, seed=7, jitter=1.5, grain=0.05),
        "camera-shake": CameraShake(
            position_amount=0.08,
            rotation_degrees=4,
            scale_amount=0.02,
            frequency=7,
            seed=7,
            attack=0.1,
            decay=3,
        ),
        "motion-blur": MotionBlur(
            intensity=2, shutter_angle=270, max_radius=32, samples=16
        ),
        "motion-tile": MotionTile(
            output_width_percent=300, output_height_percent=300, mirror_edges=True
        ),
    }


def study_source(name="original", *, treated=False):
    """Choose detail, highlights or moving edges for the operation being compared."""
    study = Group(size=SCENE)
    study.add(Color("#1a242b"))
    if name in {"glow", "bloom", "chromatic-aberration"}:
        study.add(Text("LUMA / 08", font_size=56, fill="#fff1c9"), position=(0.3, 0.5))
        study.add(
            Circle(radius=58, stroke="#95e4d1", stroke_width=5), position=(0.77, 0.48)
        )
        for index in range(5):
            study.add(
                Rectangle(width=70, height=3, fill="#f9c088"),
                position=(0.77, 0.29 + index * 0.09),
                rotation=-24,
            )
    elif name == "motion-blur":
        marker = Group(size=(160, 124))
        marker.add(Rectangle(width=160, height=124, fill="#ecd08f", corner_radius=12))
        for index in range(6):
            marker.add(
                Rectangle(width=8, height=90, fill="#1a242b"),
                position=(0.15 + index * 0.14, 0.5),
            )
        moving = study.add(marker)
        for time, x in ((0, 0.16), (0.75, 0.84), (1.5, 0.16), (2.25, 0.84), (3, 0.16)):
            moving.transform.position.keyframe(time, (x, 0.5))
        if treated:
            moving.effects.add(effect_cases()[name])
    elif name == "motion-tile":
        picture = study.add(Image(PHOTO), anchor=(0, 0), scale=0.72)
        if treated:
            picture.effects.add(effect_cases()[name])
        # The outline follows the original image bounds; it is never tiled.
        outline = study.add(
            Rectangle(
                width=640, height=220, fill=None, stroke="#fff1c9", stroke_width=3
            ),
            anchor=(0, 0),
            scale=0.72,
        )
        for layer in (picture, outline):
            layer.transform.position.keyframe(0, (0.14, 0.14))
            layer.transform.position.keyframe(1.5, (0.18, 0.20))
            layer.transform.position.keyframe(3, (0.14, 0.14))
            layer.transform.rotation.keyframe(0, -6)
            layer.transform.rotation.keyframe(1.5, 6)
            layer.transform.rotation.keyframe(3, -6)
    else:
        picture = study.add(Image(PHOTO))
        if name == "sharpen":
            picture.effects.add(GaussianBlur(radius=3))
        if name == "camera-shake":
            picture.transform.scale = 1.5
            if treated:
                picture.effects.add(effect_cases()[name])
    return study


def add_study(parent, name, *, treated=True, reverse=False):
    layer = parent.add(study_source(name, treated=treated))
    if treated and name == "order":
        effects = [GaussianBlur(radius=5), OrderedDither(palette=PALETTE, scale=2)]
        for effect in reversed(effects) if reverse else effects:
            layer.effects.add(effect)
    elif treated and name not in {"motion-tile", "motion-blur", "camera-shake"}:
        effect = effect_cases()[name]
        if effect is not None:
            layer.effects.add(effect)
    return layer


def make_project(name="original", *, reverse=False):
    project = Project(size=SCENE, fps=30, duration=DURATION, output_audio=False)
    add_study(project.root, name, reverse=reverse)
    return project


def comparison_project(name, *, reverse=False):
    # Reserve a lower band for native controls at mobile viewing widths.
    height = (
        640
        if name in {"crt", "camera-shake", "motion-blur", "motion-tile"}
        else SIZE[1]
    )
    project = Project(
        size=(640, height),
        fps=30,
        duration=DURATION,
        background="#1a242b",
        output_audio=False,
    )
    for treated, y in ((False, 150), (True, 410)):
        scene = Group(size=SCENE)
        add_study(
            scene,
            name,
            treated=True if name == "order" else treated,
            reverse=(treated != reverse) if name == "order" else reverse,
        )
        project.root.add(scene, position=(0.5, y / height))
    title = "Original" if name == "original" else name.replace("-", " ").title()
    first_label = (
        ("Dither → blur" if reverse else "Blur → dither")
        if name == "order"
        else "Source"
    )
    if name == "order":
        title = "Blur → dither" if reverse else "Dither → blur"
    for text, y in ((first_label, 20), (title, 280)):
        project.root.add(
            Text(text, font_size=28, fill="#eff2eb"),
            anchor=(0, 0.5),
            position=(0.04, y / height),
        )
    if height > SIZE[1]:
        project.root.add(
            Rectangle(width=640, height=120, fill="#10191f"),
            position=(0.5, 580 / height),
        )
        project.root.add(
            Rectangle(width=640, height=1, fill="#32434d"), position=(0.5, 520 / height)
        )
    return project


def save_still(project, path, seconds=0.7):
    frame = project.render_frame(seconds, backend="cpu")
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            f"{frame.width}x{frame.height}",
            "-i",
            "pipe:0",
            "-frames:v",
            "1",
            "-quality",
            "88",
            str(path),
        ],
        input=frame.to_bytes(),
        check=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--effect",
        choices=[*effect_cases(), "order"],
        default="gaussian-blur",
    )
    parser.add_argument(
        "--reverse", action="store_true", help="Reverse the order comparison"
    )
    parser.add_argument(
        "--still", action="store_true", help="Write WebP instead of MP4"
    )
    args = parser.parse_args()
    project = comparison_project(args.effect, reverse=args.reverse)
    suffix = "-reverse" if args.reverse else ""
    output = Path(f"effect-{args.effect}{suffix}")
    if args.still:
        save_still(project, output.with_suffix(".webp"))
    else:
        result = project.render(
            output.with_suffix(".mp4"), backend="cpu", overwrite=True
        )
        print(f"Rendered with {result.selected_backend}")


if __name__ == "__main__":
    main()
