# Image source

`Image(path, *, sizing=None, crop=None)` accepts a non-empty `str` or
`PathLike[str]`. Its canonical source is `{"type":"image","asset":...}`
after lowering. Relative paths resolve against the project base directory only
during preflight/preparation; they remain relative in serialized projects.

`sizing` is `None`, `"original"`, `"fit"`, `"cover"`, or a `Sizing` value.
`Sizing` additionally represents the canonical `scale` and `stretch` forms.
`crop` is `None`, `Crop`, or `CropProperty`; it remains mutable and may be
animated through the property API. Intrinsic dimensions are obtained from the
asset, not authored on `Image`.

Layer `start` and `duration` place the image on the project timeline. Layers
own transform, opacity, blend mode, scalar animation, effects, and transition
placement. Images support direct transform and transition endpoints. Canonical
JSON, high-level Python, CPU, and WGPU have image paths. Missing or unreadable
assets fail preflight rather than constructor validation.
