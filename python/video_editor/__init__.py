"""Deprecated compatibility imports for :mod:`vestra`."""

import vestra as _vestra

# Keep this list independent so the compatibility package owns its public
# export declaration while still forwarding new canonical exports.
__all__ = list(_vestra.__all__)
globals().update({name: getattr(_vestra, name) for name in __all__})

# The legacy name must continue to mean the immutable native project even
# after ``vestra.Project`` becomes the mutable high-level authoring API.
Project = _vestra.ProjectSnapshot
