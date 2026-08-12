//! Prepared backend selection and auto-fallback policy.

use std::sync::Arc;

#[cfg(test)]
use std::cell::RefCell;

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
    #[cfg(test)]
    if let Some(error) = test_wgpu_preparation_failure() {
        return create_backend_with(
            preference,
            || {
                vestra_render::create_backend(
                    vestra_render::RenderBackendPreference::Cpu,
                    plan,
                    decoded,
                )
                .expect("test CPU backend construction")
                .0
            },
            || Err(error),
        );
    }

    let preference = match preference {
        RenderBackendPreference::Auto => vestra_render::RenderBackendPreference::Auto,
        RenderBackendPreference::Cpu => vestra_render::RenderBackendPreference::Cpu,
        RenderBackendPreference::Wgpu => vestra_render::RenderBackendPreference::Wgpu,
    };
    vestra_render::create_backend(preference, plan, decoded).map(|(backend, fallback)| {
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

#[cfg(test)]
thread_local! {
    static TEST_WGPU_PREPARATION_FAILURE: RefCell<Option<Diagnostic>> = const { RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct TestWgpuPreparationFailureGuard(Option<Diagnostic>);

#[cfg(test)]
impl Drop for TestWgpuPreparationFailureGuard {
    fn drop(&mut self) {
        TEST_WGPU_PREPARATION_FAILURE.with(|failure| {
            *failure.borrow_mut() = self.0.take();
        });
    }
}

#[cfg(test)]
pub(crate) fn inject_wgpu_preparation_failure(
    error: Diagnostic,
) -> TestWgpuPreparationFailureGuard {
    let previous =
        TEST_WGPU_PREPARATION_FAILURE.with(|failure| failure.borrow_mut().replace(error));
    TestWgpuPreparationFailureGuard(previous)
}

#[cfg(test)]
fn test_wgpu_preparation_failure() -> Option<Diagnostic> {
    TEST_WGPU_PREPARATION_FAILURE.with(|failure| failure.borrow().clone())
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
