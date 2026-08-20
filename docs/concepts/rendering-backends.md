# Rendering backends

Vestra exposes a CPU renderer and a WGPU renderer behind the same project and
render model. The backend preference can be `auto`, `cpu`, or `wgpu` in Python,
and the CLI exposes the same choices.

There are three useful facts to keep separate:

1. The requested backend is what the caller asked for.
2. The selected backend is what Vestra prepared.
3. A WGPU adapter describes the device that actually backs that selection.

Choosing WGPU selects the graphics backend path. It does not prove that the
adapter is a hardware GPU. WGPU can run through a software adapter such as
llvmpipe. When Vestra exposes adapter metadata or a fallback warning, inspect
that information instead of inferring hardware acceleration from the word
"wgpu".

`auto` lets Vestra choose according to the available backends and preflight
rules. `cpu` is the predictable choice for a first render and for environments
where a hardware adapter is not required. Request `wgpu` when you need that
backend path, then check the selected backend and adapter in the render result,
report, or logs.

CPU and WGPU are intended to implement the same supported project semantics.
Backend-specific availability and hardware validation are operational concerns,
not changes to the authoring model.
