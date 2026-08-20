# Add a backend

A render backend is a cross-cutting implementation of the prepared/evaluated plan, not an alternate project format. It must implement the renderer abstraction used by `vestra-render`, accept prepared decoded resources, render evaluated sources/effects/groups into owned RGBA output, report metrics and errors, and cooperate with the SDK frame loop.

Use CPU and WGPU as the two current examples. Resource preparation belongs with the backend. It may allocate CPU surfaces or GPU textures, but it must not open project paths or repeat semantic validation. Frame submission must preserve layer/effect order, source adaptation, nested composition timing, cancellation checks and public frame ownership. Any asynchronous backend also needs bounded in-flight work and safe resource retirement.

Wire selection through the SDK preflight and preparation policy. Keep requested preference, selected backend and fallback distinct in `PreparationReport` and `RenderResult`. Define adapter/device information only where it has a truthful analogue. Errors must map to preparation/render diagnostics with enough stage context for reports, and progress events remain owned by the common render loop.

Before documenting support, test canonical scenes through the new backend against the supported semantics, including source/effect coverage, nested compositions, output encoding, cancellation and fallback/unavailability. Document every missing feature as `unsupported`, `partially supported` or `not fully verified`; do not imply parity because the backend compiles.
