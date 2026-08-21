"""Central lowering dispatch for high-level source values."""

from __future__ import annotations

import os
from collections.abc import Callable
from dataclasses import dataclass, replace
from typing import TYPE_CHECKING, Any, cast

from .authoring.assets import AudioAsset, ImageAsset, FontAsset, VideoAsset
from .authoring.builder import ProjectBuilder
from .authoring.clips import GroupClip, ImageClip, VideoClip, TransitionCapableClip, VisualClip
from .authoring.values import BlendMode, Color as AuthoringColor
from .sources import Circle, Color, Ellipse, Group, Image, Line, ParticleSystem, Polygon, Rectangle, Shape, Source, Spectrum2D, Text, Video
from .audio import AudioEffectStack, AudioTimeline
from .effects import EffectStack
from .flashes import FlashCollection
from .transitions import TransitionCollection
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
        self._video_asset_ids: dict[str, VideoAsset] = {}
        self._font_asset_ids: dict[str, FontAsset] = {}
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
        if not self._reserved_root_ids:
            self._reserved_root_ids = {layer.id for layer in composition.layers}
        self._transition_endpoints.update(
            endpoint
            for transition in composition.transitions.items
            for endpoint in (transition.outgoing, transition.incoming)
        )
        clips: list[VisualClip] = []
        for layer in composition.layers:
            clips.append(self._lower_layer_parts(layer, scope))
        for layer in composition.layers:
            clip = self.layer_clips[layer]
            self._lower_matte(layer, clip, scope)
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
            group_clip = self.builder.add_group_clip(
                clips=children,
                start=layer.start,
                duration=layer.duration,
                layer=layer.z,
                visible=layer.visible,
                opacity=layer.opacity.value,
                id=native_id,
            )
            self._lower_transitions(layer.child.transitions, group_clip)
            _lower_presentation(layer, group_clip, include_transform=True)
            group_clip._set_masks(self._lower_masks(layer))
            _lower_visual_effects(layer.effects, group_clip.effects)
            self.layer_clips[layer] = group_clip
            return group_clip

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
        clip: VisualClip = handler(
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
            outer._set_masks(self._lower_masks(layer))
            clip = outer
        else:
            _lower_presentation(
                layer, clip, include_transform=capabilities.supports_direct_transform
            )
            clip._set_masks(self._lower_masks(layer))
        _lower_visual_effects(layer.effects, clip.effects)
        _lower_preset(layer, clip)
        self.layer_clips[layer] = clip
        return clip

    def _lower_matte(self, layer: "Layer", clip: VisualClip, scope: tuple[str, ...]) -> None:
        if layer._matte is None:
            return
        matte, mode, invert = layer._matte
        source_id = self._local_ids.get((scope, matte.id))
        if source_id is None:
            raise ValueError(f"matte layer {matte.id!r} was not lowered in this composition")
        clip._set_matte(source_id, mode.value, invert)

    def lower_overlays(
        self, transitions: TransitionCollection, flashes: FlashCollection
    ) -> None:
        """Lower root transitions and flashes after all endpoint clips exist."""
        self._lower_transitions(transitions)
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

    def _lower_transitions(
        self, transitions: TransitionCollection, target: GroupClip | None = None
    ) -> None:
        collection = self.builder.transitions if target is None else target.transitions
        for placement in transitions.items:
            outgoing = cast(TransitionCapableClip, self.layer_clips[placement.outgoing])
            incoming = cast(TransitionCapableClip, self.layer_clips[placement.incoming])
            collection.add_transition(
                outgoing=outgoing,
                incoming=incoming,
                start=placement.start,
                duration=placement.duration,
                definition=placement.definition.to_canonical(),
                id=placement.id,
            )

    def image_asset(self, source: Image) -> ImageAsset:
        source_key = ("image", os.path.normpath(source.path))
        asset = self._asset_ids.get(source_key)
        if asset is None:
            asset = self.builder.add_image_asset(source.path)
            self._asset_ids[source_key] = asset
        return asset

    def _lower_masks(self, layer: "Layer") -> list[dict[str, object]]:
        result = []
        for mask in layer.masks.items:
            if isinstance(mask.input, Image):
                result.append(mask.to_canonical(asset_id=self.image_asset(mask.input).id, owner_duration=layer.duration))
                continue
            value = mask.to_canonical(owner_duration=layer.duration)
            input_value = cast(dict[str, object], value["input"])
            self._register_mask_input_assets(mask.input, input_value)
            result.append(value)
        return result

    def _register_mask_input_assets(self, source: Source, input_value: dict[str, object]) -> None:
        if input_value.get("type") == "image":
            input_value["asset"] = self.image_asset(cast(Image, source)).id
            return
        if input_value.get("type") != "source":
            return
        source_value = cast(dict[str, object], input_value["source"])
        self._register_mask_source_assets(source, source_value)

    def _register_mask_source_assets(self, source: Source, value: dict[str, object]) -> None:
        """Rewrite asset-bearing leaves in an owned source tree."""
        if isinstance(source, Image):
            value["asset"] = self.image_asset(source).id
        elif isinstance(source, Video):
            value["asset"] = self.video_asset(source).id
        elif isinstance(source, Text):
            value["font"] = self.font_asset(source).id
        elif isinstance(source, Group):
            clips = cast(list[dict[str, object]], value.get("clips", []))
            for child, clip in zip(source.children, clips):
                child_value = cast(dict[str, object], clip["source"])
                self._register_mask_source_assets(child.source, child_value)
                for child_mask, child_mask_value in zip(
                    child.masks.items,
                    cast(list[dict[str, object]], clip.get("masks", [])),
                ):
                    self._register_mask_input_assets(
                        child_mask.input,
                        cast(dict[str, object], child_mask_value["input"]),
                    )

    def video_asset(self, source: Video) -> VideoAsset:
        source_key = os.path.normpath(source.path)
        asset = self._video_asset_ids.get(source_key)
        if asset is None:
            asset = self.builder.add_video_asset(source.path)
            self._video_asset_ids[source_key] = asset
        return asset

    def font_asset(self, source: Text) -> FontAsset:
        key = os.path.normpath(source.font)
        asset = self._font_asset_ids.get(key)
        if asset is None:
            asset = self.builder.add_font_asset(source.font)
            self._font_asset_ids[key] = asset
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


def _lower_video(
    context: LoweringContext, layer: Layer, native_id: str, placement: Placement
) -> VisualClip:
    source = cast(Video, layer.source)
    crop = source.crop.value if source.crop.active else None
    clip = context.builder.add_video_clip(
        source=context.video_asset(source),
        start=placement.start,
        duration=placement.duration,
        source_start=layer.source_start,
        playback_rate=layer.playback_rate,
        layer=placement.layer,
        visible=placement.visible,
        sizing=source.sizing,
        crop=crop,
        opacity=placement.opacity,
        id=native_id,
    )
    return clip


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


def _lower_shape(
    context: LoweringContext, layer: Layer, native_id: str, placement: Placement
) -> VisualClip:
    source = cast(Shape, layer.source)
    return context.builder.add_shape_clip(
        shape=source.to_canonical(),
        start=placement.start,
        duration=placement.duration,
        layer=placement.layer,
        visible=placement.visible,
        opacity=placement.opacity,
        id=native_id,
    )


def _lower_text(
    context: LoweringContext, layer: Layer, native_id: str, placement: Placement
) -> VisualClip:
    source = cast(Text, layer.source)
    return context.builder.add_text_clip(
        source=source.to_canonical(), font=context.font_asset(source), start=placement.start,
        duration=placement.duration, layer=placement.layer, visible=placement.visible,
        opacity=placement.opacity, id=native_id,
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
    if isinstance(layer.source, Video) and isinstance(clip, VideoClip):
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
    Video,
    _lower_video,
    SourceCapabilities(
        has_intrinsic_duration=True,
        supports_direct_transform=True,
        supports_direct_transition_endpoint=True,
        supports_sizing=True,
        supports_crop=True,
    ),
)
register_source(
    Color, _lower_color, SourceCapabilities(supports_transition_adapter=True)
)
for _shape_type in (Shape, Rectangle, Ellipse, Circle, Line, Polygon):
    register_source(
        _shape_type,
        _lower_shape,
        SourceCapabilities(
            supports_direct_transform=True,
            supports_direct_transition_endpoint=True,
        ),
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
register_source(Text, _lower_text, SourceCapabilities(
    supports_direct_transform=True, supports_direct_transition_endpoint=True,
))


__all__ = [
    "LoweringContext",
    "Placement",
    "SourceCapabilities",
    "register_source",
    "source_capabilities",
]
