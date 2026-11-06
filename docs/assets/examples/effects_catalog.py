"""Render controlled effect comparisons, using only procedural Vestra sources.

Run with --effect gaussian-blur (or any key in effect_cases()). Outputs are
written to the current directory; rerunning replaces the named output.
"""

import argparse
import colorsys
from pathlib import Path
import subprocess

from vestra import Circle, Color, Group, Project, Rectangle, Text
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

SIZE = (640, 360)
PALETTE = ("#101b32", "#476b80", "#d6a46b", "#fff2cf")


def effect_cases():
    """Each preview changes one effect; values are deliberately visible at 640 px."""
    return {
        "original": None,
        "brightness": Brightness(amount=0.15),
        "contrast": Contrast(amount=1.5),
        "saturation": Saturation(amount=0),
        "tint": Tint(colour="#4ed5bf", amount=0.65),
        "color-adjust": ColorAdjust(
            exposure=0.5, gamma=1.3, black_point=0, white_point=1
        ),
        "gaussian-blur": GaussianBlur(radius=5),
        "directional-blur": DirectionalBlur(radius=16, angle_degrees=25),
        "zoom-blur": ZoomBlur(radius=18, samples=24, anchor=(0.5, 0.5)),
        "radial-blur": RadialBlur(amount=12, center=(0.5, 0.5)),
        "glow": Glow(threshold=0.65, radius=10, intensity=1.5, colour="#8adfff"),
        "bloom": Bloom(threshold=0.65, radius=10, intensity=1.5),
        "chromatic-aberration": ChromaticAberration(amount=6, angle_degrees=0),
        "vignette": Vignette(amount=0.8, radius=0.5, softness=0.4, colour="#000000"),
        "sharpen": Sharpen(amount=2, radius=2),
        "palette-map": PaletteMap(palette=PALETTE, mode="nearest"),
        "ordered-dither": OrderedDither(palette=PALETTE, matrix="bayer8", scale=1),
        "blue-noise": OrderedDither(palette=PALETTE, matrix="blue_noise", seed=37),
        "halftone": Halftone(mode="source", cell_size=7, softness=0.5),
        "pixel-sort": PixelSort(direction="horizontal", segment_length=96, amount=1),
        "ascii": Ascii(
            cell_width=8, cell_height=12, color_mode="source", source_mix=0.15
        ),
        "pseudo-ascii": PseudoAscii(cell_width=8, cell_height=12, color_mode="source"),
        "crt": Crt(period=2, seed=7, jitter=1.5, grain=0.05),
        "camera-shake": CameraShake(
            position_amount=0.025,
            rotation_degrees=2,
            scale_amount=0.02,
            frequency=5,
            seed=7,
            attack=0.1,
            decay=0.2,
        ),
        "motion-blur": MotionBlur(
            intensity=1, shutter_angle=270, max_radius=32, samples=16
        ),
        "motion-tile": MotionTile(
            output_width_percent=200, output_height_percent=200, mirror_edges=True
        ),
    }


def study_source():
    """Tonal ramps, fine lines and broad shapes reveal different kinds of change."""
    study = Group(size=SIZE)
    study.add(Color("#101b32"))
    for column in range(32):
        red, green, blue = colorsys.hsv_to_rgb(
            column / 40, 0.65, 0.2 + 0.8 * column / 31
        )
        colour = f"#{int(red * 255):02x}{int(green * 255):02x}{int(blue * 255):02x}"
        study.add(
            Rectangle(width=20, height=190, fill=colour),
            position=((column * 20 + 10) / 640, 0.36),
        )
    for index in range(16):
        shade = round(255 * index / 15)
        study.add(
            Rectangle(width=40, height=34, fill=f"#{shade:02x}{shade:02x}{shade:02x}"),
            position=((index * 40 + 20) / 640, 0.88),
        )
    study.add(Circle(radius=65, fill="#fff2cf"), position=(0.26, 0.4))
    study.add(Circle(radius=45, fill="#101b32"), position=(0.29, 0.37))
    for index in range(8):
        study.add(
            Rectangle(width=2, height=100, fill="#ffffff"),
            position=(0.62 + index * 0.024, 0.4),
            rotation=20,
        )
    study.add(Text("Edges / 0123", font_size=32), position=(0.5, 0.73))
    return study


def make_project(name="original", *, reverse=False):
    project = Project(size=SIZE, fps=30, duration=2)
    layer = project.root.add(study_source())
    if name in {
        "motion-tile",
        "camera-shake",
        "motion-blur",
        "motion-original",
        "tile-original",
    }:
        layer.transform.scale = 0.7 if name in {"motion-tile", "tile-original"} else 1.1
        layer.transform.rotation.keyframe(0, -12)
        layer.transform.rotation.keyframe(2, 12)
        layer.transform.position.keyframe(0, (0.35, 0.5))
        layer.transform.position.keyframe(2, (0.65, 0.5))
    if name == "order":
        effects = [GaussianBlur(radius=5), OrderedDither(palette=PALETTE, scale=2)]
        for effect in reversed(effects) if reverse else effects:
            layer.effects.add(effect)
    elif name not in {"motion-original", "tile-original"}:
        effect = effect_cases()[name]
        if effect is not None:
            layer.effects.add(effect)
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
        choices=[*effect_cases(), "order", "motion-original", "tile-original"],
        default="gaussian-blur",
    )
    parser.add_argument(
        "--reverse", action="store_true", help="Reverse the order comparison"
    )
    parser.add_argument(
        "--still", action="store_true", help="Write WebP instead of MP4"
    )
    args = parser.parse_args()
    project = make_project(args.effect, reverse=args.reverse)
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
