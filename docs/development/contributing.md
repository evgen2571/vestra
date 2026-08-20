# Contributing

Keep changes small and preserve crate boundaries. Use CodeGraph first when an unfamiliar subsystem is involved, then use targeted source/test inspection. Run formatting and relevant checks before proposing a change. Public Rust/Python APIs, stubs, canonical JSON, schema, CLI contracts, reports, time conversions, and diagnostics are compatibility-sensitive.

Keep CPU and WGPU semantics aligned, place FFmpeg/process details in media I/O, and update tests and current documentation with a public contract change. Use [testing](testing.md) and [GPU validation](gpu-validation.md) for commands and hardware policy.
