"""Central lowering dispatch for high-level source values."""

from __future__ import annotations

import os
from collections.abc import Callable
from dataclasses import dataclass, replace
from typing import TYPE_CHECKING, Any, cast

from .authoring.assets import AudioAsset, ImageAsset
from .authoring.builder import ProjectBuilder
from .authoring.clips import GroupClip, ImageClip, VisualClip
from .authoring.values import BlendMode, Color as AuthoringColor
from .sources import Color, Image, ParticleSystem, Source, Spectrum2D
from .audio import AudioEffectStack, AudioTimeline
from .effects import EffectStack
from .flashes import FlashCollection
from .transitions import (
    Crossfade,
    DirectionalPush,
    FlashCut,
    TransitionCollection,
    ZoomBlur,
    ZoomCrossfade,
)
from .properties import (
    BindablePointProperty,
    BindableScalarProperty,
    CropProperty,
    PointProperty,
    ScalarBindingTarget,
    ScalarProperty,
    Transform,
)

if TYPE_CHECKING:
    from .editor import Composition, Layer


@dataclass(frozen=True, slots=True)
class SourceCapabilities:
    """Facts used by lowering and the extension contract.

    Direct transform, direct transition endpoint, transition adapter, and
    cinematic preset are consumed by current editor/lowering paths. Sizing and
    crop describe native source ownership and are retained for source-specific
    lowering and future source families. Intrinsic duration and audio
    requirements are deliberately future-facing descriptors: Video and richer
    audio-reactive sources can use them without adding source-type branches to
    ``Composition``.
    """

    has_intrinsic_duration: bool = False
    supports_direct_transform: bool = False
    supports_direct_transition_endpoint: bool = False
    supports_transition_adapter: bool = False
    supports_sizing: bool = False
    supports_crop: bool = False
    supports_cinematic_preset: bool = False
    requires_audio: bool = False


Handler = Callable[["LoweringContext", "Layer", str, "Placement"], "VisualClip"]
_REGISTRY: dict[type[Source], tuple[Handler, SourceCapabilities]] = {}


@dataclass(frozen=True, slots=True)
class Placement:
    """Immutable timing/presentation placement supplied to source handlers."""

    start: float
    duration: float
    layer: int
    visible: bool
    opacity: float
    blend_mode: BlendMode

    @classmethod
    def from_layer(cls, layer: "Layer", *, neutral: bool = False) -> "Placement":
        if neutral:
            return cls(0.0, layer.duration, 0, True, 1.0, BlendMode.NORMAL)
        return cls(
            layer.start,
            layer.duration,
            layer.z,
            layer.visible,
            layer.opacity.value,
            layer.blend_mode,
        )


def register_source(
    source_type: type[Source],
    handler: Handler,
    capabilities: SourceCapabilities | None = None,
) -> None:
    if not isinstance(source_type, type) or not issubclass(source_type, Source):
        raise TypeError("source_type must be a Source subclass")
    if source_type in _REGISTRY:
        raise ValueError(
            f"source lowering already registered for {source_type.__name__}"
        )
    _REGISTRY[source_type] = (handler, capabilities or SourceCapabilities())


def source_capabilities(source: Source | type[Source]) -> SourceCapabilities:
    source_type = source if isinstance(source, type) else type(source)
    try:
        capabilities = _REGISTRY[source_type][1]
    except KeyError as error:
        raise TypeError(
            f"source type {source_type.__name__} is not registered"
        ) from error
    if isinstance(source, ParticleSystem) and source.audio_reactive is not None:
        return replace(capabilities, requires_audio=True)
    return capabilities


def _escape_component(value: str) -> str:
    """Escape scope separators while keeping ordinary authored IDs readable."""
    return value.replace("~", "~0").replace("/", "~1")


class LoweringContext:
    """Own IDs, scopes, clip mappings, recursion, and asset deduplication."""

    def __init__(self, builder: ProjectBuilder) -> None:
        self.builder = builder
        self._asset_ids: dict[tuple[str, str], ImageAsset] = {}
        self._audio_asset_ids: dict[str, AudioAsset] = {}
        self._local_ids: dict[tuple[tuple[str, ...], str], str] = {}
        self.layer_clips: dict[Layer, VisualClip] = {}
        self._reserved_root_ids: set[str] = set()
        self._used_native_ids: set[str] = set()
        self._transition_endpoints: set[Layer] = set()

    def _native_id(self, parts: tuple[str, ...], identifier: str) -> str:
        if not parts:
            return identifier
        base = "/".join(
            (
                *(_escape_component(part) for part in parts),
                _escape_component(identifier),
            )
        )
        candidate = base
        suffix = 2
        while (
            candidate in self._reserved_root_ids or candidate in self._used_native_ids
        ):
            candidate = f"{base}~{suffix}"
            suffix += 1
        return candidate

    def lower_composition(
        self, composition: Composition, *, scope: tuple[str, ...] = ()
    ) -> list[VisualClip]:
        if not scope and not self._reserved_root_ids:
            self._reserved_root_ids = {layer.id for layer in composition.layers}
            self._transition_endpoints = {
                endpoint
                for transition in composition.transitions._items
                for endpoint in transition[:2]
            }
        clips: list[VisualClip] = []
        for layer in composition.layers:
            clips.append(self._lower_layer_parts(layer, scope))
        return clips

    def _lower_layer_parts(self, layer: Layer, scope: tuple[str, ...]) -> VisualClip:
        key = (scope, layer.id)
        if key in self._local_ids:
            raise ValueError(f"duplicate layer ID: {layer.id!r}")
        self._local_ids[key] = self._native_id(scope, layer.id)
        native_id = self._local_ids[key]
        self._used_native_ids.add(native_id)

        # This is a graph node check, not a source-specific lowering branch.
        from .editor import CompositionLayer

        if isinstance(layer, CompositionLayer):
            children = self.lower_composition(layer.child, scope=(*scope, layer.id))
            clip: VisualClip = self.builder.add_group_clip(
                clips=children,
                start=layer.start,
                duration=layer.duration,
                layer=layer.z,
                visible=layer.visible,
                opacity=layer.opacity.value,
                id=native_id,
            )
            _lower_presentation(layer, clip, include_transform=True)
            _lower_visual_effects(layer.effects, clip.effects)
            self.layer_clips[layer] = clip
            return clip

        registered = _REGISTRY.get(type(layer.source))
        if registered is None:
            raise TypeError(
                f"no lowering registered for source type {type(layer.source).__name__}"
            )
        handler, capabilities = registered
        needs_adapter = (
            not capabilities.supports_direct_transform
            and not layer.transform.is_default()
        ) or (
            layer in self._transition_endpoints
            and not capabilities.supports_direct_transition_endpoint
        )
        clip = handler(
            self, layer, native_id, Placement.from_layer(layer, neutral=needs_adapter)
        )
        if needs_adapter:
            # Unsupported sources are adapted through a presentation-only outer
            # group.  The source keeps local timing and default presentation.
            outer = self.builder.add_group_clip(
                clips=[clip],
                start=layer.start,
                duration=layer.duration,
                layer=layer.z,
                visible=layer.visible,
                opacity=layer.opacity.value,
                id=native_id,
            )
            _lower_presentation(layer, outer, include_transform=True)
            clip = outer
        else:
            _lower_presentation(
                layer, clip, include_transform=capabilities.supports_direct_transform
            )
        _lower_visual_effects(layer.effects, clip.effects)
        _lower_preset(layer, clip)
        self.layer_clips[layer] = clip
        return clip

    def lower_overlays(
        self, transitions: TransitionCollection, flashes: FlashCollection
    ) -> None:
        """Lower root transitions and flashes after all endpoint clips exist."""
        for outgoing_layer, incoming_layer, transition in transitions._items:
            outgoing = cast(ImageClip | GroupClip, self.layer_clips[outgoing_layer])
            incoming = cast(ImageClip | GroupClip, self.layer_clips[incoming_layer])
            if isinstance(transition, Crossfade):
                self.builder.transitions.add_crossfade(
                    outgoing=outgoing,
                    incoming=incoming,
                    start=transition.start,
                    duration=transition.duration,
                    interpolation=transition.interpolation,
                    id=transition.id,
                )
            elif isinstance(transition, ZoomCrossfade):
                self.builder.transitions.add_zoom_crossfade(
                    outgoing=outgoing,
                    incoming=incoming,
                    start=transition.start,
                    duration=transition.duration,
                    interpolation=transition.interpolation,
                    id=transition.id,
                    outgoing_zoom=transition.outgoing_zoom,
                    incoming_start_zoom=transition.incoming_start_zoom,
                )
            elif isinstance(transition, FlashCut):
                self.builder.transitions.add_flash_cut(
                    outgoing=outgoing,
                    incoming=incoming,
                    start=transition.start,
                    duration=transition.duration,
                    interpolation=transition.interpolation,
                    id=transition.id,
                    colour=cast(str, transition.colour),
                    intensity=transition.intensity,
                )
            elif isinstance(transition, DirectionalPush):
                self.builder.transitions.add_directional_push(
                    outgoing=outgoing,
                    incoming=incoming,
                    start=transition.start,
                    duration=transition.duration,
                    interpolation=transition.interpolation,
                    id=transition.id,
                    angle_degrees=transition.angle_degrees,
                    distance=transition.distance,
                    blur_radius=transition.blur_radius,
                )
            elif isinstance(transition, ZoomBlur):
                self.builder.transitions.add_zoom_blur(
                    outgoing=outgoing,
                    incoming=incoming,
                    start=transition.start,
                    duration=transition.duration,
                    interpolation=transition.interpolation,
                    id=transition.id,
                    outgoing_zoom=transition.outgoing_zoom,
                    incoming_start_zoom=transition.incoming_start_zoom,
                    blur_radius=transition.blur_radius,
                )
            else:
                raise TypeError(
                    f"unsupported transition type: {type(transition).__name__}"
                )
        for flash in flashes.items:
            self.builder.flashes.add(
                start=flash.start,
                duration=flash.duration,
                colour=flash.colour,
                opacity=flash.opacity,
                fade_in=flash.fade_in,
                fade_out=flash.fade_out,
                layer=flash.layer,
                id=flash.id,
            )

    def image_asset(self, source: Image) -> ImageAsset:
        source_key = ("image", os.path.normpath(source.path))
        asset = self._asset_ids.get(source_key)
        if asset is None:
            asset = self.builder.add_image_asset(source.path)
            self._asset_ids[source_key] = asset
        return asset

    def audio_asset(self, path: str) -> AudioAsset:
        """Register each normalized audio path once for this snapshot."""
        source_key = os.path.normpath(path)
        asset = self._audio_asset_ids.get(source_key)
        if asset is None:
            asset = self.builder.add_audio_asset(path)
            self._audio_asset_ids[source_key] = asset
        return asset

    def lower_audio(self, timeline: AudioTimeline) -> None:
        """Lower the high-level audio graph into fresh builder-owned nodes."""
        for track in timeline.tracks:
            native_track = self.builder.audio.add_track(
                id=track.id, mute=track.mute, gain=track.gain
            )
            _lower_audio_effects(track.effects, native_track.effects)
            for clip in track.clips:
                native_clip = native_track.add_clip(
                    asset=self.audio_asset(clip.path),
                    id=clip.id,
                    start=clip.start,
                    trim_start=clip.trim_start,
                    trim_end=clip.trim_end,
                    mute=clip.mute,
                    gain=clip.gain,
                    fade_in=clip.fade_in,
                    fade_out=clip.fade_out,
                    fade_in_curve=clip.fade_in_curve,
                    fade_out_curve=clip.fade_out_curve,
                )
                if clip.gain_automation:
                    native_clip.set_gain_automation(clip.gain_automation)
                _lower_audio_effects(clip.effects, native_clip.effects)
        _lower_audio_effects(timeline.effects, self.builder.audio.effects)

    def lower_post_effects(self, effects: EffectStack) -> None:
        """Lower root visual post-effects independently from the audio graph."""
        _lower_visual_effects(effects, self.builder.post_effects)


def _lower_audio_effects(source: AudioEffectStack, target: Any) -> None:
    """Dispatch all high-level effects through one native generic entry point."""
    for effect in source.items:
        target.add_effect(effect.type, **effect.parameters())


def _lower_visual_effects(source: EffectStack, target: Any) -> None:
    """Lower high-level effect descriptors into fresh native-owned tracks."""
    for effect in source.items:
        native = target.add_effect(
            effect.type, id=effect.id, **effect._native_parameters()
        )
        for name, property_value in effect._property_items():
            _lower_scalar_property(property_value, native.parameter_track(name))


def _lower_preset(layer: Layer, clip: VisualClip) -> None:
    preset = layer.presets.current
    if preset is None:
        return
    if not isinstance(clip, ImageClip):
        raise TypeError("cinematic presets are supported only on Image layers")
    arguments: dict[str, object] = {
        "intensity": preset.intensity,
        "start": preset.start,
        "duration": preset.duration,
    }
    if preset.seed is not None:
        arguments["seed"] = preset.seed
    getattr(clip.presets, f"apply_{preset.kind}")(**arguments)


def _lower_image(
    context: LoweringContext, layer: Layer, native_id: str, placement: Placement
) -> VisualClip:
    source = cast(Image, layer.source)
    crop = source.crop.value if source.crop.active else None
    return context.builder.add_image_clip(
        source=context.image_asset(source),
        start=placement.start,
        duration=placement.duration,
        layer=placement.layer,
        visible=placement.visible,
        sizing=source.sizing,
        crop=crop,
        opacity=placement.opacity,
        id=native_id,
    )


def _lower_color(
    context: LoweringContext, layer: Layer, native_id: str, placement: Placement
) -> VisualClip:
    source = cast(Color, layer.source)
    return context.builder.add_solid_color_clip(
        colour=AuthoringColor(source.value),
        start=placement.start,
        duration=placement.duration,
        layer=placement.layer,
        visible=placement.visible,
        opacity=placement.opacity,
        id=native_id,
    )


def _lower_particle(
    context: LoweringContext, layer: Layer, native_id: str, placement: Placement
) -> VisualClip:
    source = cast(ParticleSystem, layer.source)
    return context.builder.add_particle_system_clip(
        particle_system=source.lowering_definition(context.builder),
        start=placement.start,
        duration=placement.duration,
        layer=placement.layer,
        visible=placement.visible,
        opacity=placement.opacity,
        id=native_id,
    )


def _lower_spectrum(
    context: LoweringContext, layer: Layer, native_id: str, placement: Placement
) -> VisualClip:
    source = cast(Spectrum2D, layer.source)
    kwargs = source.lowering_kwargs()
    clip = context.builder.add_spectrum2d_clip(
        start=placement.start,
        duration=placement.duration,
        layer=placement.layer,
        visible=placement.visible,
        opacity=placement.opacity,
        id=native_id,
        preset=source.preset,
        **cast(Any, kwargs),
    )
    return cast(VisualClip, clip)


def _lower_scalar_property(source: ScalarProperty, target: Any) -> None:
    target.base_value = source.value
    target.clear_keyframes()
    for frame in source.keyframes:
        target.keyframe(
            time=frame.time, value=frame.value, interpolation=frame.interpolation
        )
    if isinstance(source, BindableScalarProperty):
        for binding in source.bindings:
            target.modulate(binding.signal, mode=binding.operation)


def _lower_point_property(source: PointProperty, target: Any) -> None:
    target.base_value = source.value
    target.clear_keyframes()
    for frame in source.keyframes:
        target.keyframe(
            time=frame.time, value=frame.value, interpolation=frame.interpolation
        )
    if isinstance(source, BindablePointProperty):
        for binding in source.bindings:
            target.react_to(binding.signal, mode=binding.operation)


def _lower_component_bindings(source: ScalarBindingTarget, target: Any) -> None:
    for binding in source.bindings:
        target.modulate(binding.signal, mode=binding.operation)


def _lower_crop_property(source: CropProperty, target: Any) -> None:
    target.base_value = source.value
    target.clear_keyframes()
    for frame in source.keyframes:
        target.keyframe(
            time=frame.time, value=frame.value, interpolation=frame.interpolation
        )


def _lower_transform(source: Transform, target: Any) -> None:
    _lower_point_property(source.position, target.transform.position)
    _lower_point_property(source.anchor, target.transform.anchor)
    _lower_point_property(source.scale, target.transform.scale)
    _lower_scalar_property(source.rotation_degrees, target.transform.rotation_degrees)
    _lower_component_bindings(source.position_x, target.transform.position_x)
    _lower_component_bindings(source.position_y, target.transform.position_y)
    _lower_component_bindings(source.scale_x, target.transform.scale_x)
    _lower_component_bindings(source.scale_y, target.transform.scale_y)


def _lower_presentation(
    layer: Layer, clip: VisualClip, *, include_transform: bool
) -> None:
    """Lower all common layer presentation values after clip construction."""
    clip.start = layer.start
    clip.duration = layer.duration
    clip.layer = layer.z
    clip.visible = layer.visible
    _lower_scalar_property(layer.opacity, clip.opacity)
    clip.blend_mode = layer.blend_mode
    if isinstance(layer.source, Image) and isinstance(clip, ImageClip):
        # Image crop is source-owned but lowered into its native clip track.
        if layer.source.crop.active:
            _lower_crop_property(layer.source.crop, clip.crop)
    if include_transform:
        _lower_transform(layer.transform, clip)


register_source(
    Image,
    _lower_image,
    SourceCapabilities(
        supports_direct_transform=True,
        supports_direct_transition_endpoint=True,
        supports_sizing=True,
        supports_crop=True,
        supports_cinematic_preset=True,
    ),
)
register_source(
    Color, _lower_color, SourceCapabilities(supports_transition_adapter=True)
)
register_source(
    ParticleSystem,
    _lower_particle,
    SourceCapabilities(supports_transition_adapter=True),
)
register_source(
    Spectrum2D,
    _lower_spectrum,
    SourceCapabilities(requires_audio=True, supports_transition_adapter=True),
)


__all__ = [
    "LoweringContext",
    "Placement",
    "SourceCapabilities",
    "register_source",
    "source_capabilities",
]
