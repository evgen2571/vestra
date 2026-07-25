//! Prepared backend selection and auto-fallback policy.

use std::sync::Arc;

use crate::{
    Diagnostic,
    plan::RenderPlan,
    render::{CpuBackend, RenderBackend, WgpuBackend, prepared::DecodedAssets},
};

use super::engine_types::{BackendFallback, RenderBackendPreference};

#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves structured diagnostics for auto fallback and explicit requests"
)]
pub(super) fn create_backend(
    preference: RenderBackendPreference,
    plan: &RenderPlan,
    decoded: &Arc<DecodedAssets>,
) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic> {
    if let Err(error) = super::wgpu_support::validate_plan(plan) {
        return match preference {
            RenderBackendPreference::Wgpu => Err(error),
            RenderBackendPreference::Auto => Ok((
                Box::new(CpuBackend::new(plan, Arc::clone(decoded))),
                Some(BackendFallback {
                    code: error.code,
                    stage: "effect_capability".to_owned(),
                    message: error.message,
                }),
            )),
            RenderBackendPreference::Cpu => {
                Ok((Box::new(CpuBackend::new(plan, Arc::clone(decoded))), None))
            }
        };
    }
    create_backend_with(
        preference,
        Box::new(CpuBackend::new(plan, Arc::clone(decoded))),
        || WgpuBackend::new(plan, Arc::clone(decoded)).map(|backend| Box::new(backend) as _),
    )
}

#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves structured diagnostics for auto fallback and explicit requests"
)]
pub(super) fn create_backend_with<F>(
    preference: RenderBackendPreference,
    cpu: Box<dyn RenderBackend>,
    create_wgpu: F,
) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic>
where
    F: FnOnce() -> Result<Box<dyn RenderBackend>, Diagnostic>,
{
    match preference {
        RenderBackendPreference::Cpu => Ok((cpu, None)),
        RenderBackendPreference::Wgpu => Ok((create_wgpu()?, None)),
        RenderBackendPreference::Auto => match create_wgpu() {
            Ok(backend) => Ok((backend, None)),
            Err(error) => Ok((
                cpu,
                Some(BackendFallback {
                    code: error.code,
                    stage: "wgpu_preparation".to_owned(),
                    message: error.message,
                }),
            )),
        },
    }
}
