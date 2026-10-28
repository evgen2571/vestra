//! Backend-owned kernel capability policy.
use crate::{
    Category, Diagnostic,
    kernel::{EffectKernel, required_effect_kernels},
};
use vestra_core::plan::RenderPlan;

pub(crate) use super::effects::supports_kernel;

pub(crate) fn validate_plan(plan: &RenderPlan) -> Result<(), Diagnostic> {
    validate_required_kernels_with(required_effect_kernels(plan).iter(), supports_kernel)
}

fn validate_required_kernels_with(
    kernels: impl IntoIterator<Item = EffectKernel>,
    supports: impl Fn(EffectKernel) -> bool,
) -> Result<(), Diagnostic> {
    for kernel in kernels {
        if !supports(kernel) {
            return Err(Diagnostic::error(
                "RENDER-UNSUPPORTED-KERNEL",
                Category::Backend,
                format!("cpu backend does not support effect kernel {kernel:?}"),
                "",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_declared_kernel_is_supported_by_cpu() {
        assert!(EffectKernel::ALL.into_iter().all(supports_kernel));
        assert!(supports_kernel(EffectKernel::Composite));
    }
    #[test]
    fn unsupported_kernels_preserve_the_backend_diagnostic() {
        let error =
            validate_required_kernels_with([EffectKernel::Composite], |_| false).unwrap_err();
        assert_eq!(error.code, "RENDER-UNSUPPORTED-KERNEL");
        assert_eq!(
            error.message,
            "cpu backend does not support effect kernel Composite"
        );
    }
}
