from vestra import Project, ScalarProperty
from vestra.effects import (
    Halftone, PixelSort, Crt, HalftoneMode, PixelSortDirection, PixelSortOrder,
    Bloom,
    Brightness,
    CameraShake,
    ChromaticAberration,
    ColorAdjust,
    Contrast,
    DirectionalBlur,
    GaussianBlur,
    Glow,
    MotionBlur,
    PaletteMap,
    OrderedDither,
    PaletteMode,
    DitherMatrix,
    Saturation,
    Sharpen,
    Tint,
    Vignette,
    ZoomBlur,
)
from vestra.authoring.values import Point
from vestra.sources import Color

project = Project(size=(2, 2), fps=1, duration=1)
layer = project.root.add(Color("#ffffff"))
signal = project.audio.signal.rms()
layer.effects.add(Brightness(0)).amount.bind(signal)
layer.effects.add(Contrast(1)).amount.keyframe(0, 1)
layer.effects.add(Saturation(1)).amount = ScalarProperty(1)
layer.effects.add(Tint("#ffffff", 0.5)).amount.bind(signal)
layer.effects.add(GaussianBlur(1)).radius.bind(signal)
layer.effects.add(DirectionalBlur(1, 0)).angle_degrees.bind(signal)
layer.effects.add(ZoomBlur(1, 2, Point(0.5, 0.5)))
layer.effects.add(Glow(0.5, 1, 1, "#ffffff"))
layer.effects.add(Bloom(0.5, 1, 1))
layer.effects.add(ChromaticAberration(1, 45)).amount.bind(signal)
layer.effects.add(Vignette(0.5, 1, 0.5, "#000000")).softness.keyframe(0, 0.5)
layer.effects.add(Sharpen(0.5, 1))
layer.effects.add(ColorAdjust(0, 1, 0.2, 0.8))
layer.effects.add(CameraShake(1, 1, 0.1, 1, 1, 0, 1))
layer.effects.add(MotionBlur(1, 180, 2, 2))

layer.effects.add(PaletteMap(["#000000", "#ffffff"], mode=PaletteMode.GRADIENT, period=2)).phase.bind(signal)
layer.effects.add(OrderedDither(matrix=DitherMatrix.BAYER4, scale=2)).strength.keyframe(1, 0.5)

layer.effects.add(Halftone(mode=HalftoneMode.SOURCE)).cell_size.bind(signal)
layer.effects.add(PixelSort(direction=PixelSortDirection.VERTICAL, order=PixelSortOrder.DESCENDING)).amount.keyframe(0, 0.5)
project.post_effects.add(Crt(period=2)).phase.bind(signal)

layer.effects.add(OrderedDither(matrix=DitherMatrix.BLUE_NOISE, seed=37)).seed = 42

layer.effects.add(PaletteMap(mode=PaletteMode.NEAREST_RGB))
layer.effects.add(OrderedDither(mode=PaletteMode.NEAREST_HUE))
layer.effects.add(OrderedDither(mode=PaletteMode.RGB_CHANNELS, levels=6)).levels = 3

layer.effects.add(PaletteMap(mode=PaletteMode.NEAREST_OKLAB))

layer.effects.add(PaletteMap(["#000000", "#ff0000", "#ffffff"], stops=(0, 0.25, 1))).stops = None
layer.effects.add(OrderedDither(stops=(0, 1))).stops = (0.0, 1.0)

layer.effects.add(PaletteMap(interpolation="oklab")).interpolation = "rgb"
layer.effects.add(OrderedDither(interpolation="oklab"))

layer.effects.add(PaletteMap(input_gamma=1.5)).input_exposure.bind(signal)
layer.effects.add(OrderedDither(input_exposure=0.5)).input_gamma.keyframe(1, 2)

layer.effects.add(PaletteMap(input_detail=1, input_detail_radius=8)).input_detail.bind(signal)
layer.effects.add(OrderedDither(input_detail=1.5)).input_detail_radius.keyframe(1, 2)

layer.effects.add(PaletteMap(input_scale=4, input_filter="area")).input_scale.bind(project.audio.signal.rms())
layer.effects.add(OrderedDither(input_scale=4, input_filter="linear")).input_filter = "nearest"
