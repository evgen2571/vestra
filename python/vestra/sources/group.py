"""Small source-local groups usable as owned mask inputs."""

from __future__ import annotations

from collections.abc import Iterable
from copy import deepcopy
from typing import TYPE_CHECKING, Any, cast

from .base import Source
from .image import Image
from .video import Video
from ..authoring.values import BlendMode
from ..effects import EffectStack
from ..properties import BindableScalarProperty, ScalarProperty, Transform
from ..properties.lowering import lower_effect, lower_scalar_property, lower_transform

if TYPE_CHECKING:
    from ..masks import MaskCollection


class GroupChild:
    """A source-local placement owned by :class:`Group`."""

    __slots__ = (
        "source", "start", "duration", "source_start", "playback_rate",
        "layer", "visible", "_opacity", "transform", "_effects", "_blend_mode",
        "_masks",
    )

    def __init__(
        self,
        source: Source,
        *,
        start: int | float = 0.0,
        duration: int | float | None = None,
        source_start: int | float = 0.0,
        playback_rate: int | float = 1.0,
        layer: int = 0,
        visible: bool = True,
        opacity: int | float = 1.0,
    ) -> None:
        if not isinstance(source, Source):
            raise TypeError("Group child source must be a vestra source value")
        if isinstance(start, bool) or not isinstance(start, int | float) or start < 0:
            raise ValueError("Group child start must be non-negative")
        if duration is not None and (isinstance(duration, bool) or not isinstance(duration, int | float) or duration <= 0):
            raise ValueError("Group child duration must be positive")
        if isinstance(source_start, bool) or not isinstance(source_start, int | float) or source_start < 0:
            raise ValueError("Group child source_start must be non-negative")
        if isinstance(playback_rate, bool) or not isinstance(playback_rate, int | float) or playback_rate <= 0:
            raise ValueError("Group child playback_rate must be positive")
        if isinstance(layer, bool) or not isinstance(layer, int):
            raise TypeError("Group child layer must be an integer")
        if not isinstance(visible, bool):
            raise TypeError("Group child visible must be a boolean")
        if isinstance(opacity, bool) or not isinstance(opacity, int | float) or not 0 <= opacity <= 1:
            raise ValueError("Group child opacity must be between 0 and 1")
        self.source = source.snapshot()
        self.start = float(start)
        self.duration = None if duration is None else float(duration)
        self.source_start = float(source_start)
        self.playback_rate = float(playback_rate)
        self.layer = layer
        self.visible = visible
        self._opacity = BindableScalarProperty(opacity, minimum=0.0, maximum=1.0)
        self.transform = Transform()
        self._effects = EffectStack("layer")
        self._blend_mode = BlendMode.NORMAL
        self._masks: MaskCollection | None = None

    @property
    def masks(self) -> "MaskCollection":
        if self._masks is None:
            from ..masks import MaskCollection

            self._masks = MaskCollection()
        return self._masks

    @property
    def effects(self) -> EffectStack:
        return self._effects

    @property
    def opacity(self) -> BindableScalarProperty:
        return self._opacity

    @opacity.setter
    def opacity(self, value: int | float | ScalarProperty) -> None:
        if isinstance(value, ScalarProperty):
            value._copy_to(self._opacity)
        else:
            self._opacity.value = value

    @property
    def blend_mode(self) -> BlendMode:
        return self._blend_mode

    @blend_mode.setter
    def blend_mode(self, value: BlendMode) -> None:
        if not isinstance(value, BlendMode):
            raise TypeError("blend_mode must be BlendMode")
        self._blend_mode = value

    def snapshot(self) -> "GroupChild":
        return deepcopy(self)


class Group(Source):
    """A source-local composition of visual sources.

    Children are source-local Clips. Their timing and presentation are local to
    the Group and are lowered through the normal canonical Clip fields.
    """

    __slots__ = ("_children",)

    def __init__(self, children: Iterable[Source] = ()) -> None:
        values = tuple(children)
        if not all(isinstance(child, Source | GroupChild) for child in values):
            raise TypeError("Group children must be source values or GroupChild objects")
        self._children = tuple(
            child.snapshot() if isinstance(child, GroupChild) else GroupChild(child)
            for child in values
        )

    @property
    def children(self) -> tuple[GroupChild, ...]:
        return self._children

    def add(
        self,
        source: Source,
        *,
        start: int | float = 0.0,
        duration: int | float | None = None,
        source_start: int | float = 0.0,
        playback_rate: int | float = 1.0,
        layer: int | None = None,
        visible: bool = True,
        opacity: int | float = 1.0,
    ) -> GroupChild:
        child = GroupChild(
            source,
            start=start,
            duration=duration,
            source_start=source_start,
            playback_rate=playback_rate,
            layer=len(self._children) if layer is None else layer,
            visible=visible,
            opacity=opacity,
        )
        self._children = (*self._children, child)
        return child

    def to_canonical(self, *, inherited_duration: float | None = None) -> dict[str, object]:
        clips = []
        for index, child in enumerate(self.children):
            duration = child.duration if child.duration is not None else inherited_duration
            if duration is None:
                raise ValueError("Group child duration requires a containing Group lifetime")
            source = (
                child.source.to_canonical(inherited_duration=duration)
                if isinstance(child.source, Group)
                else cast(Any, child.source).to_canonical()
            )
            if isinstance(source, str):
                source = {"type": "solid_color", "colour": source}
            effects = []
            for effect_index, effect in enumerate(child.effects.items):
                effect_value = lower_effect(effect)
                effect_value.setdefault("id", f"group-child-{index}-effect-{effect_index}")
                effects.append(effect_value)
            clip: dict[str, object] = {
                "id": f"group-child-{index}",
                "start": child.start,
                "duration": duration,
                "layer": child.layer,
                "visible": child.visible,
                "opacity": lower_scalar_property(child.opacity),
                "effects": effects,
                "source": source,
                "transform": lower_transform(child.transform),
            }
            if child.blend_mode is not BlendMode.NORMAL:
                clip["blend_mode"] = child.blend_mode.to_canonical()
            if isinstance(child.source, (Image, Video)):
                if child.source.sizing is not None:
                    clip["sizing"] = child.source.sizing.to_canonical()
                if child.source.crop.active:
                    clip["crop"] = child.source.crop.to_canonical()
            if isinstance(child.source, Video):
                clip["source_start"] = child.source_start
                if child.playback_rate != 1.0:
                    clip["playback_rate"] = child.playback_rate
            if child.masks.items:
                clip["masks"] = [mask.to_canonical(owner_duration=duration) for mask in child.masks.items]
            clips.append(clip)
        return {"type": "group", "clips": clips}


__all__ = ["Group", "GroupChild"]
