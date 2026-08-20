# Project to plan

`vestra-core` deserializes the canonical project and validates its structural and semantic relationships. SDK preflight then resolves relative assets, probes media, determines final duration/frame count, checks output readiness, and probes a requested backend. Compilation converts validated project data into renderer-independent visual, transition, effect, audio, and signal plans. Frame-time evaluation applies timeline time, tracks, signals, and transition progress.

Compilation centralizes cross-clip semantics and timing checks so renderers receive evaluated work rather than reinterpret public project data. Diagnostics keep JSON pointers and operation context across those boundaries.
