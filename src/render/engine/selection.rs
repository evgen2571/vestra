//! Prepared backend selection and auto-fallback policy.

use std::sync::Arc;

use crate::{
    Diagnostic,
    plan::RenderPlan,
    render::{DecodedAssets, RenderBackend},
};

use super::types::{BackendFallback, RenderBackendPreference};

#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves structured diagnostics for auto fallback and explicit requests"
)]
pub(super) fn create_backend(
    preference: RenderBackendPreference,
    plan: &RenderPlan,
    decoded: &Arc<DecodedAssets>,
) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic> {
    let preference = match preference {
        RenderBackendPreference::Auto => video_editor_render::RenderBackendPreference::Auto,
        RenderBackendPreference::Cpu => video_editor_render::RenderBackendPreference::Cpu,
        RenderBackendPreference::Wgpu => video_editor_render::RenderBackendPreference::Wgpu,
    };
    video_editor_render::create_backend(preference, plan, decoded).map(|(backend, fallback)| {
        (
            backend,
            fallback.map(|fallback| BackendFallback {
                code: fallback.code,
                stage: fallback.stage,
                message: fallback.message,
            }),
        )
    })
}

#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves structured diagnostics for auto fallback and explicit requests"
)]
#[cfg(test)]
pub(super) fn create_backend_with<CF, WF>(
    preference: RenderBackendPreference,
    create_cpu: CF,
    create_wgpu: WF,
) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic>
where
    CF: FnOnce() -> Box<dyn RenderBackend>,
    WF: FnOnce() -> Result<Box<dyn RenderBackend>, Diagnostic>,
{
    match preference {
        RenderBackendPreference::Cpu => Ok((create_cpu(), None)),
        RenderBackendPreference::Wgpu => Ok((create_wgpu()?, None)),
        RenderBackendPreference::Auto => match create_wgpu() {
            Ok(backend) => Ok((backend, None)),
            Err(error) => Ok((
                create_cpu(),
                Some(BackendFallback {
                    code: error.code,
                    stage: "wgpu_preparation".to_owned(),
                    message: error.message,
                }),
            )),
        },
    }
}
