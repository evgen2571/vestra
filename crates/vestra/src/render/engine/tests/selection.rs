use super::*;

#[test]
fn cpu_selection_never_attempts_wgpu_initialization() {
    let (backend, fallback) =
        create_backend_with(RenderBackendPreference::Cpu, selection_backend, || {
            panic!("CPU selection must not initialize WGPU")
        })
        .expect("CPU backend selection succeeds");

    assert_eq!(backend.kind(), RenderBackendKind::Cpu);
    assert!(fallback.is_none());
}

#[test]
fn auto_selection_falls_back_with_the_wgpu_diagnostic() {
    let (backend, fallback) =
        create_backend_with(RenderBackendPreference::Auto, selection_backend, || {
            Err(Diagnostic::error(
                "WGPU-ADAPTER-NOT-FOUND",
                Category::Backend,
                "injected adapter failure",
                "",
            ))
        })
        .expect("automatic selection falls back");

    assert_eq!(backend.kind(), RenderBackendKind::Cpu);
    assert!(matches!(
        fallback,
        Some(BackendFallback { code, stage, message })
            if code == "WGPU-ADAPTER-NOT-FOUND"
                && stage == "wgpu_preparation"
                && message == "injected adapter failure"
    ));
}
