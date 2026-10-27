//! Feature-selected facade behavior and compatibility contracts.
use super::*;
use std::sync::Arc;

fn prepared() -> (vestra_core::plan::RenderPlan, Arc<DecodedAssets>) {
    let input = test_support::load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &test_support::ValidationOptions::default(),
    )
    .unwrap();
    let plan = vestra_core::plan::compile(input, Default::default()).unwrap();
    let decoded = DecodedAssets::build(&plan).unwrap();
    (plan, decoded)
}

#[test]
fn cpu_probe_reports_the_compiled_feature() {
    let probe = probe_backend(RenderBackendPreference::Cpu);
    assert_eq!(
        probe.selected,
        cfg!(feature = "cpu").then_some(RenderBackendPreference::Cpu)
    );
    assert!(probe.fallback.is_none());
    assert!(probe.diagnostics.is_empty());
}

#[test]
fn explicit_cpu_selection_preserves_the_feature_contract() {
    let (plan, decoded) = prepared();
    let result = create_backend(RenderBackendPreference::Cpu, &plan, &decoded);
    #[cfg(feature = "cpu")]
    {
        let (backend, fallback) = result.unwrap();
        assert_eq!(backend.kind(), RenderBackendKind::Cpu);
        assert!(fallback.is_none());
    }
    #[cfg(not(feature = "cpu"))]
    {
        let error = result.err().expect("CPU is unavailable");
        assert_eq!(error.code, "VESTRA-CPU-UNAVAILABLE");
        assert_eq!(error.message, "CPU support is not enabled in this build");
        assert_eq!(error.category, Category::Backend);
    }
}

#[cfg(not(feature = "wgpu"))]
#[test]
fn explicit_wgpu_selection_preserves_the_unavailable_diagnostic() {
    let (plan, decoded) = prepared();
    let error = create_backend(RenderBackendPreference::Wgpu, &plan, &decoded)
        .err()
        .expect("WGPU is unavailable");
    assert_eq!(error.code, "VESTRA-WGPU-UNAVAILABLE");
    assert_eq!(error.message, "WGPU support is not enabled in this build");
    assert_eq!(error.category, Category::Backend);
}

#[cfg(all(feature = "cpu", not(feature = "wgpu")))]
#[test]
fn cpu_only_auto_selection_needs_no_fallback() {
    let (plan, decoded) = prepared();
    let (backend, fallback) =
        create_backend(RenderBackendPreference::Auto, &plan, &decoded).unwrap();
    assert_eq!(backend.kind(), RenderBackendKind::Cpu);
    assert!(fallback.is_none());
}

#[cfg(not(any(feature = "cpu", feature = "wgpu")))]
#[test]
fn no_backend_auto_selection_preserves_the_unavailable_diagnostic() {
    let (plan, decoded) = prepared();
    let error = create_backend(RenderBackendPreference::Auto, &plan, &decoded)
        .err()
        .expect("no backend is compiled");
    assert_eq!(error.code, "VESTRA-BACKEND-UNAVAILABLE");
    assert_eq!(
        error.message,
        "no renderer backend is enabled in this build"
    );
    assert_eq!(error.category, Category::Backend);
}
