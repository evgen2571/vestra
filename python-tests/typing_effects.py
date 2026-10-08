from vestra import Project, ScalarProperty
from vestra.effects import (
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
