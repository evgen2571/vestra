"""Public palette effects preserve color, animation, and ownership contracts."""

from __future__ import annotations

import pytest

import vestra
from vestra import effects
from vestra.authoring import ProjectBuilder
from vestra.authoring.values import Color


def effect_class(name: str):
    assert hasattr(effects, name), f"{name} must be a public effect"
    return getattr(effects, name)


@pytest.mark.parametrize(
    "name,kind,mode",
    [
        ("PaletteMap", "palette_map", "gradient"),
        ("OrderedDither", "ordered_dither", "nearest"),
    ],
)
def test_defaults_lower_to_valid_native_effects(name, kind, mode):
    project = vestra.Project(size=(4, 4), fps=2, duration=1)
    layer = project.root.add(vestra.sources.Color("#808080"), duration=1)
    layer.effects.add(effect_class(name)())
    snapshot = project.snapshot()
    data = snapshot.to_dict()["visual"]["clips"][0]["effects"][0]
    assert data["type"] == kind
    assert data["palette"] == ["#000000", "#ffffff"]
    assert data["mode"] == mode
    assert data["amount"]["base_value"] == 1
    assert data["phase"]["base_value"] == 0
    assert data.get("period") is None
    if kind == "ordered_dither":
        assert data["strength"]["base_value"] == 1
        assert data["matrix"] == "bayer8"
        assert data["scale"] == 1
    assert (
        vestra.ProjectSnapshot.from_json(snapshot.to_json()).to_dict()
        == snapshot.to_dict()
    )


@pytest.mark.parametrize("name", ["PaletteMap", "OrderedDither"])
def test_palette_inputs_and_outputs_do_not_share_mutable_state(name):
    palette = [Color("#001122"), "#FFEECC"]
    effect = effect_class(name)(palette)
    palette[0] = "#ffffff"
    assert effect.palette == ("#001122", "#ffeecc")
    canonical = effect.to_canonical()
    canonical["palette"][0] = "#ffffff"
    assert effect.palette[0] == "#001122"
    stack = effects.EffectStack("layer")
    attached = stack.add(effect)
    effect.palette = ["#000000", "#ffffff"]
    assert attached.palette == ("#001122", "#ffeecc")
    attached.phase.keyframe(1, 0.5)
    assert not effect.phase.keyframes


@pytest.mark.parametrize("name", ["PaletteMap", "OrderedDither"])
@pytest.mark.parametrize(
    "palette",
    [
        [],
        ["#000000"],
        ["#ffffff"] * 17,
        ["#000000", "#ffffff80"],
        ["#xyzxyz", "#ffffff"],
    ],
)
def test_invalid_palettes_are_rejected_atomically(name, palette):
    cls = effect_class(name)
    with pytest.raises((TypeError, ValueError)):
        cls(palette)
    effect = cls()
    before = effect.palette
    with pytest.raises((TypeError, ValueError)):
        effect.palette = palette
    assert effect.palette == before


@pytest.mark.parametrize("name", ["PaletteMap", "OrderedDither"])
@pytest.mark.parametrize("period", [0, -1, float("nan"), float("inf"), True])
def test_period_must_be_finite_positive_seconds(name, period):
    cls = effect_class(name)
    with pytest.raises((TypeError, ValueError)):
        cls(period=period)
    effect = cls(period=2)
    with pytest.raises((TypeError, ValueError)):
        effect.period = period
    assert effect.period == 2
    effect.period = None
    assert effect.to_canonical().get("period") is None


def test_palette_modes_and_dither_matrices_use_their_own_enums():
    cls = effect_class("PaletteMap")
    dither_cls = effect_class("OrderedDither")
    palette = cls(mode="rainbow")
    assert palette.mode is effects.PaletteMode.RAINBOW
    palette.mode = effects.PaletteMode.NEAREST
    dither = dither_cls(matrix="bayer4")
    assert dither.matrix is effects.DitherMatrix.BAYER4
    dither.matrix = effects.DitherMatrix.BAYER2
    for field, invalid in [
        ("mode", "rgb"),
        ("matrix", "random"),
        ("scale", 0),
        ("scale", 33),
        ("scale", True),
    ]:
        with pytest.raises((TypeError, ValueError)):
            setattr(dither, field, invalid)
    assert dither.to_canonical()["matrix"] == "bayer2"


@pytest.mark.parametrize("name", ["PaletteMap", "OrderedDither"])
def test_scalar_animation_and_signals_survive_high_level_lowering(name):
    project = vestra.Project(size=(4, 4), fps=2, duration=2)
    layer = project.root.add(vestra.sources.Color("#808080"), duration=2)
    effect = layer.effects.add(effect_class(name)(period=2))
    effect.amount.keyframe(0.5, 0.25)
    effect.phase.keyframe(1, 0.5)
    effect.phase.bind(project.audio.signal.rms(), operation="add")
    if name == "OrderedDither":
        effect.strength.keyframe(1, 0.75)
        effect.strength.bind(project.audio.signal.rms(), operation="multiply")
    snapshot = project.snapshot()
    data = snapshot.to_dict()["visual"]["clips"][0]["effects"][0]
    assert data["amount"]["keyframes"][0]["value"] == 0.25
    assert data["phase"]["keyframes"][0]["value"] == 0.5
    assert data["phase"]["modifiers"][0]["operation"] == "add"
    if name == "OrderedDither":
        assert data["strength"]["modifiers"][0]["operation"] == "multiply"
    effect.amount.value = 0.8
    assert (
        snapshot.to_dict()["visual"]["clips"][0]["effects"][0]["amount"]["base_value"]
        == 1
    )


@pytest.mark.parametrize(
    "kind,factory",
    [("palette_map", "add_palette_map"), ("ordered_dither", "add_ordered_dither")],
)
def test_advanced_typed_and_generic_authoring_have_matching_contracts(kind, factory):
    builder = ProjectBuilder(
        width=4,
        height=4,
        frame_rate=vestra.FrameRate(2, 1),
        output_path="out.mp4",
        duration=2,
    )
    clip = builder.add_solid_color_clip(colour="#808080", start=0, duration=2, layer=0)
    assert hasattr(clip.effects, factory)
    typed = getattr(clip.effects, factory)(
        palette=["#000000", "#88ffcc"], mode="rainbow", period=2
    )
    typed.phase.keyframe(time=1, value=0.5)
    generic_parameters = {
        "palette": ["#000000", "#88ffcc"],
        "mode": "rainbow",
        "amount": 1,
        "phase": typed.phase,
        "period": 2,
    }
    if kind == "ordered_dither":
        generic_parameters.update(strength=1, matrix="bayer8", scale=1)
    generic = builder.post_effects.add_effect(kind, **generic_parameters)
    typed_data, generic_data = typed.to_canonical(), generic.to_canonical()
    typed_data.pop("id")
    generic_data.pop("id")
    assert typed_data == generic_data
    assert builder.validate().is_valid
    typed.palette = ["#001122", "#ffffff"]
    typed.period = None
    assert typed.to_canonical()["palette"] == ["#001122", "#ffffff"]
    assert generic.to_canonical()["palette"] == ["#000000", "#88ffcc"]
    with pytest.raises(ValueError):
        builder.post_effects.add_effect(kind, **{**generic_parameters, "period": 0})


@pytest.mark.parametrize("name", ["PaletteMap", "OrderedDither"])
def test_constructor_copies_scalar_property_animation(name):
    amount = vestra.ScalarProperty(0.5)
    amount.keyframe(1, 0.75)
    effect = effect_class(name)(amount=amount)
    amount.value = 0
    assert effect.amount.value == 0.5
    assert effect.amount.keyframes[0].value == 0.75
    with pytest.raises(ValueError):
        effect_class(name)(amount=vestra.ScalarProperty(2))


def test_advanced_palette_serialization_does_not_expose_builder_state():
    builder = ProjectBuilder(
        width=4,
        height=4,
        frame_rate=vestra.FrameRate(2, 1),
        output_path="out.mp4",
        duration=1,
    )
    clip = builder.add_solid_color_clip(colour="#808080", start=0, duration=1, layer=0)
    palette = ["#000000", "#ffffff"]
    effect = clip.effects.add_palette_map(palette=palette)
    palette[0] = "#112233"
    effect.to_canonical()["palette"][0] = "#aabbcc"
    assert effect.palette == ("#000000", "#ffffff")
    assert builder.to_dict()["visual"]["clips"][0]["effects"][0]["palette"] == [
        "#000000",
        "#ffffff",
    ]


@pytest.mark.parametrize("kind", ["palette_map", "ordered_dither"])
def test_advanced_palette_failures_preserve_collection_and_ids(kind):
    builder = ProjectBuilder(
        width=4,
        height=4,
        frame_rate=vestra.FrameRate(2, 1),
        output_path="out.mp4",
        duration=1,
    )
    clip = builder.add_solid_color_clip(colour="#808080", start=0, duration=1, layer=0)
    arguments = {"amount": 1, "phase": 0}
    if kind == "ordered_dither":
        arguments.update(strength=1, scale=1)
    for palette in ([], ["#000000", "#ffffff80"]):
        with pytest.raises(ValueError):
            clip.effects.add_effect(kind, id="look", palette=palette, **arguments)
        assert not clip.effects.items
    effect = clip.effects.add_effect(
        kind, id="look", palette=["#000000", "#ffffff"], **arguments
    )
    audio = builder.add_audio_asset("examples/assets/tone.wav")
    builder.audio.add_track().add_clip(asset=audio, start=0)
    effect.parameter_track("phase").modulate(builder.audio.master.rms(), mode="add")
    assert effect.to_canonical()["phase"]["modifiers"][0]["operation"] == "add"
    assert builder.validate().is_valid


@pytest.mark.parametrize("name", ["PaletteMap", "OrderedDither"])
def test_palette_effects_render_deterministic_moving_video(tmp_path, name):
    import subprocess

    # FFmpeg's generated pattern is original synthetic footage; no download.
    path = tmp_path / "moving.mkv"
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x36:rate=30",
            "-t",
            "1",
            "-an",
            "-c:v",
            "ffv1",
            "-fflags",
            "+bitexact",
            "-flags",
            "+bitexact",
            str(path),
        ],
        check=True,
    )
    project = vestra.Project(size=(64, 36), fps=30, duration=1)
    layer = project.root.add(vestra.sources.Video(str(path)), duration=1)
    effect = layer.effects.add(effect_class(name)())
    prepared = project.prepare(backend="cpu")
    first = prepared.render_frame_number(2).to_bytes()
    later = prepared.render_frame_number(20).to_bytes()
    assert first != later
    assert prepared.render_frame_number(2).to_bytes() == first
    for offset in range(0, len(first), 4):
        assert first[offset] == first[offset + 1] == first[offset + 2]
        assert first[offset + 3] == 255
        if name == "OrderedDither":
            assert first[offset] in (0, 255)
    effect.amount = 0
    identity = project.prepare(backend="cpu").render_frame_number(2).to_bytes()
    baseline_project = vestra.Project(size=(64, 36), fps=30, duration=1)
    baseline_project.root.add(vestra.sources.Video(str(path)), duration=1)
    baseline = baseline_project.prepare(backend="cpu").render_frame_number(2).to_bytes()
    assert identity == baseline


def test_blue_noise_seed_round_trips_through_both_authoring_apis():
    effect = effects.OrderedDither(matrix="blue_noise", seed=37)
    assert effect.to_canonical()["matrix"] == "blue_noise"
    assert effect.to_canonical()["seed"] == 37
    effect.seed = 4294967295
    assert effect.copy().seed == 4294967295
    with pytest.raises(ValueError):
        effect.seed = -1
    with pytest.raises(ValueError):
        effect.seed = 4294967296
    builder = ProjectBuilder(
        width=32, height=32, frame_rate=vestra.FrameRate(2, 1), output_path="out.mp4"
    )
    attached = builder.post_effects.add_ordered_dither(matrix="blue_noise", seed=37)
    assert attached.to_canonical()["seed"] == 37
    attached.seed = 5
    assert attached.to_canonical()["seed"] == 5


@pytest.mark.parametrize("mode", ["nearest_rgb", "nearest_hue", "nearest_oklab"])
@pytest.mark.parametrize("effect_class", [effects.PaletteMap, effects.OrderedDither])
def test_chromatic_quantization_preserves_literal_palette_colors(mode, effect_class):
    project = vestra.Project(size=(4, 4), fps=1, duration=1)
    layer = project.root.add(vestra.sources.Color("#00ff00"), duration=1)
    layer.effects.add(effect_class(("#ff0000", "#00ff00", "#0000ff"), mode=mode))
    pixels = project.render_frame(0, backend="cpu").to_bytes()
    assert pixels == bytes([0, 255, 0, 255]) * 16


def test_hue_metric_retains_dark_red_hue_when_rgb_prefers_gray():
    def render(mode):
        project = vestra.Project(size=(2, 2), fps=1, duration=1)
        layer = project.root.add(vestra.sources.Color("#320000"), duration=1)
        layer.effects.add(effects.PaletteMap(("#323232", "#ff0000"), mode=mode))
        return project.render_frame(0, backend="cpu").to_bytes()
    assert render("nearest_rgb") == bytes([50, 50, 50, 255]) * 4
    assert render("nearest_hue") == bytes([255, 0, 0, 255]) * 4


@pytest.mark.parametrize("effect_class", [effects.PaletteMap, effects.OrderedDither])
@pytest.mark.parametrize("levels,expected", [(2, [0, 0, 255, 255]), (4, [85, 85, 170, 255]), (256, [63, 127, 191, 255])])
def test_channel_count_quantization_has_defined_byte_rounding(effect_class, levels, expected):
    project = vestra.Project(size=(4, 4), fps=1, duration=1)
    layer = project.root.add(vestra.sources.Color("#3f7fbf"), duration=1)
    kwargs = {"strength": 0} if effect_class is effects.OrderedDither else {}
    effect = layer.effects.add(effect_class(mode="rgb_channels", levels=levels, **kwargs))
    assert effect.levels == levels
    assert project.render_frame(0, backend="cpu").to_bytes() == bytes(expected) * 16


@pytest.mark.parametrize("levels", [1, 257, 4.5, True, -1])
@pytest.mark.parametrize("effect_class", [effects.PaletteMap, effects.OrderedDither])
def test_channel_levels_are_bounded_discrete_integers(effect_class, levels):
    with pytest.raises((TypeError, ValueError)):
        effect_class(mode="rgb_channels", levels=levels)


def test_channel_levels_edit_atomically_through_both_authoring_apis():
    effect = effects.OrderedDither(mode=effects.PaletteMode.RGB_CHANNELS, levels=8)
    for invalid in (1, 257, True, 4.5):
        with pytest.raises((ValueError, TypeError)):
            effect.levels = invalid
        assert effect.levels == 8
    builder = ProjectBuilder(width=32, height=32, frame_rate=vestra.FrameRate(2, 1), output_path="out.mp4")
    for attached in (builder.post_effects.add_palette_map(mode="rgb_channels", levels=4),
                     builder.post_effects.add_ordered_dither(mode="rgb_channels", levels=4)):
        assert attached.levels == 4
        attached.levels = 256
        assert attached.to_canonical()["levels"] == 256
        with pytest.raises(ValueError):
            attached.levels = 257
        assert attached.levels == 256


def test_perceptual_mapping_uses_lightness_instead_of_encoded_rgb_distance():
    outputs = []
    for mode in ("nearest_rgb", "nearest_oklab"):
        project = vestra.Project(size=(4, 4), fps=1, duration=1)
        layer = project.root.add(vestra.sources.Color("#686868"), duration=1)
        layer.effects.add(effects.PaletteMap(mode=mode))
        outputs.append(project.render_frame(0, backend="cpu").to_bytes())
    assert outputs[0] == bytes([0, 0, 0, 255]) * 16
    assert outputs[1] == bytes([255, 255, 255, 255]) * 16


@pytest.mark.parametrize("name", ["PaletteMap", "OrderedDither"])
def test_nonuniform_stops_are_copied_editable_and_render_literal_tones(name):
    stops = [0, 64 / 255, 1]
    options = {"mode": "gradient"} if name == "PaletteMap" else {"strength": 0}
    effect = effect_class(name)(["#000000", "#ff0000", "#ffffff"], stops=stops, **options)
    stops[1] = 0.5
    assert effect.stops == (0.0, 64 / 255, 1.0)
    canonical = effect.to_canonical()
    canonical["stops"][1] = 0.5
    assert effect.stops[1] == 64 / 255
    project = vestra.Project(size=(4, 4), fps=2, duration=1)
    layer = project.root.add(vestra.sources.Color("#202020"), duration=1)
    layer.effects.add(effect)
    assert project.render_frame(0, backend="cpu").to_bytes() == bytes([128, 0, 0, 255] if name == "PaletteMap" else [255, 0, 0, 255]) * 16
    effect.stops = None
    assert "stops" not in effect.to_canonical()


@pytest.mark.parametrize("name", ["PaletteMap", "OrderedDither"])
@pytest.mark.parametrize("stops", [[], [0], [0, 0.2, 0.1, 1], [0.1, 1], [0, 0.9], [0, 0, 1], [0, 0.000001, 1], [0, float("nan"), 1], [0, float("inf"), 1], [False, 1], "0,1"])
def test_nonuniform_stop_inputs_fail_predictably(name, stops):
    with pytest.raises((ValueError, TypeError)):
        effect_class(name)(stops=stops)


@pytest.mark.parametrize("mode", ["rainbow", "nearest_rgb", "nearest_hue", "rgb_channels", "nearest_oklab"])
def test_nonuniform_stops_reject_modes_without_tonal_positions(mode):
    project = vestra.Project(size=(4, 4), fps=2, duration=1)
    layer = project.root.add(vestra.sources.Color("#808080"), duration=1)
    layer.effects.add(effects.OrderedDither(mode=mode, stops=(0, 1)))
    assert not project.validate().is_valid


@pytest.mark.parametrize("factory,kind", [("add_palette_map", "palette_map"), ("add_ordered_dither", "ordered_dither")])
def test_advanced_nonuniform_stops_match_generic_and_native_validation(factory, kind):
    builder = ProjectBuilder(width=4, height=4, frame_rate=vestra.FrameRate(2, 1), output_path="out.mp4", duration=1)
    builder.add_solid_color_clip(colour="#808080", start=0, duration=1, layer=0)
    options = {"palette": ["#000000", "#ff0000", "#ffffff"], "mode": "nearest", "stops": [0, 0.25, 1]}
    typed = getattr(builder.post_effects, factory)(**options)
    generic_options = {**options, "amount": 1, "phase": 0}
    if kind == "ordered_dither":
        generic_options.update(strength=1, scale=1)
    generic = builder.post_effects.add_effect(kind, **generic_options)
    assert typed.stops == (0, 0.25, 1)
    assert typed.to_canonical()["stops"] == generic.to_canonical()["stops"]
    assert builder.validate().is_valid
    typed.stops = (0, 1)
    assert not builder.validate().is_valid
    typed.stops = None
    assert "stops" not in typed.to_canonical()
    assert builder.validate().is_valid
