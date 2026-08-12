//! Shared adapter setup for WGPU tests.

use std::sync::Arc;

use super::WgpuBackend;

pub(super) fn wgpu_backend_or_skip(
    plan: &crate::plan::RenderPlan,
    decoded: Arc<crate::DecodedAssets>,
) -> Option<WgpuBackend> {
    match WgpuBackend::new(plan, decoded) {
        Ok(backend) => {
            eprintln!("WGPU_RUNTIME_EXECUTED adapter=available backend=wgpu");
            Some(backend)
        }
        Err(error) if std::env::var_os("VESTRA_REQUIRE_WGPU").is_some() => {
            panic!(
                "strict WGPU verification requires an adapter and device: {}",
                error.message
            )
        }
        Err(error) if is_environment_unavailable(&error) => {
            eprintln!(
                "WGPU_RUNTIME_SKIPPED reason=no-compatible-adapter code={} message={}",
                error.code, error.message
            );
            None
        }
        Err(error) => panic!(
            "WGPU backend initialization failed after adapter discovery: {}",
            error.message
        ),
    }
}

pub(super) fn wgpu_backend_or_skip_depth(
    plan: &crate::plan::RenderPlan,
    decoded: Arc<crate::DecodedAssets>,
    depth: usize,
) -> Option<WgpuBackend> {
    match WgpuBackend::new_with_pipeline_depth(plan, decoded, depth) {
        Ok(backend) => {
            eprintln!("WGPU_RUNTIME_EXECUTED adapter=available backend=wgpu");
            Some(backend)
        }
        Err(error) if std::env::var_os("VESTRA_REQUIRE_WGPU").is_some() => {
            panic!(
                "strict WGPU verification requires an adapter and device: {}",
                error.message
            )
        }
        Err(error) if is_environment_unavailable(&error) => {
            eprintln!(
                "WGPU_RUNTIME_SKIPPED reason=no-compatible-adapter code={} depth={depth} message={}",
                error.code, error.message
            );
            None
        }
        Err(error) => panic!(
            "WGPU backend initialization failed after adapter discovery: {}",
            error.message
        ),
    }
}

fn is_environment_unavailable(error: &crate::Diagnostic) -> bool {
    matches!(
        error.code.as_str(),
        "WGPU-ADAPTER-NOT-FOUND" | "WGPU-NO-COMPATIBLE-ADAPTER"
    )
}
