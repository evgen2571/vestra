# Python bindings

The high-level Python editing layer owns mutable `Project`, `Composition`, `Layer`, and source handles. Snapshot/lowering produces canonical data, which crosses PyO3 to the public Rust SDK. Native `ProjectSnapshot`, `Editor`, preparation, render requests, events, results, diagnostics, and cancellation objects are immutable/runtime controls.

Native preparation and rendering detach from the GIL. Progress callbacks reacquire Python only for callbacks; callback failures map to render failure rather than a successful result. The package stubs describe the exported runtime boundary.
