//! Shared adapter setup for WGPU tests.

use std::sync::Arc;

use super::WgpuBackend;

pub(super) fn wgpu_backend_or_skip(
    plan: &crate::plan::RenderPlan,
    decoded: Arc<crate::DecodedAssets>,
) -> Option<WgpuBackend> {
    match WgpuBackend::new(plan, decoded) {
        Ok(backend) => Some(backend),
        Err(error) if std::env::var_os("VIDEO_EDITOR_REQUIRE_WGPU").is_some() => {
            panic!(
                "strict WGPU verification requires an adapter and device: {}",
                error.message
            )
        }
        Err(error) => {
            eprintln!("skipping adapter-dependent WGPU test: {}", error.message);
            None
        }
    }
}

pub(super) fn wgpu_backend_or_skip_depth(
    plan: &crate::plan::RenderPlan,
    decoded: Arc<crate::DecodedAssets>,
    depth: usize,
) -> Option<WgpuBackend> {
    match WgpuBackend::new_with_pipeline_depth(plan, decoded, depth) {
        Ok(backend) => Some(backend),
        Err(error) if std::env::var_os("VIDEO_EDITOR_REQUIRE_WGPU").is_some() => {
            panic!(
                "strict WGPU verification requires an adapter and device: {}",
                error.message
            )
        }
        Err(error) => {
            eprintln!(
                "skipping adapter-dependent WGPU depth {depth} test: {}",
                error.message
            );
            None
        }
    }
}
