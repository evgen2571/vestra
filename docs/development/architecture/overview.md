# Architecture overview

Vestra keeps project semantics independent of rendering. `vestra-core` owns the canonical model, validation, time/plan compilation, evaluation, signals, and descriptor catalogs. `vestra-render` turns evaluated plans into CPU or WGPU frames. `vestra-media` owns probing, decode, audio graphs, sinks, encoding, and publication. `vestra` exposes the public SDK and coordinates loading, validation, preflight, preparation, and rendering. `vestra-python` bridges that SDK through PyO3. `vestra-cli` presents it as `ve` with logging and reports.

The usual path is project input, canonical validation, environment preflight, plan/resource preparation, backend selection, frame/audio processing, encoding, temporary-output finalization, publication, and a result or diagnostic.
