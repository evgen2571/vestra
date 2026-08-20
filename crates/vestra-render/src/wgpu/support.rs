//! Exhaustive WGPU plan compatibility policy.

use crate::{
    Diagnostic,
    backend::RenderBackendKind,
    kernel::{required_effect_kernels, validate_required_kernels},
    plan::RenderPlan,
    project::BlendMode,
};

#[cfg(test)]
use crate::kernel::EffectKernel;

/// Validate the renderer-owned mappings before adapter creation.  Adapter
/// limits remain the responsibility of `GpuRequirements`, but a future effect
/// or blend variant cannot silently enter the WGPU path without a mapping.
#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves the existing structured diagnostic"
)]
pub(crate) fn validate_plan(plan: &RenderPlan) -> Result<(), Diagnostic> {
    for layer in &plan.layers {
        validate_blend_mode(layer.blend_mode)?;
    }
    validate_required_kernels(
        RenderBackendKind::Wgpu,
        required_effect_kernels(plan).iter(),
    )?;
    Ok(())
}

#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves the existing structured diagnostic"
)]
#[cfg(test)]
pub(crate) fn validate_kernel_capabilities(
    backend: RenderBackendKind,
    kernels: impl IntoIterator<Item = EffectKernel>,
) -> Result<(), Diagnostic> {
    validate_kernel_capabilities_with(backend, kernels, |kernel| {
        crate::kernel::supports_kernel(backend, kernel)
    })
}

#[cfg(test)]
fn validate_kernel_capabilities_with(
    backend: RenderBackendKind,
    kernels: impl IntoIterator<Item = EffectKernel>,
    supports: impl Fn(EffectKernel) -> bool,
) -> Result<(), Diagnostic> {
    for kernel in kernels {
        if !supports(kernel) {
            return Err(Diagnostic::error(
                "RENDER-UNSUPPORTED-KERNEL",
                crate::Category::Backend,
                format!(
                    "{} backend does not support effect kernel {kernel:?}",
                    backend.as_str()
                ),
                "",
            ));
        }
    }
    Ok(())
}

fn validate_blend_mode(mode: BlendMode) -> Result<(), Diagnostic> {
    match mode {
        BlendMode::Normal
        | BlendMode::Add
        | BlendMode::Screen
        | BlendMode::Multiply
        | BlendMode::Overlay => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_kernel_capabilities, validate_kernel_capabilities_with, validate_plan};
    use crate::{
        backend::RenderBackendKind,
        kernel::{EffectKernel, required_effect_kernels},
    };
    use crate::{
        plan::{CompileOptions, compile},
        project::{ValidationOptions, load_and_validate},
    };

    #[test]
    fn current_advanced_transition_preset_and_post_effect_plan_does_not_force_cpu_fallback() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/effects-ready.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("effects fixture validates");
        let plan =
            compile(&validated, CompileOptions::default()).expect("effects fixture compiles");
        assert!(plan.compilation.generated_transform_contribution_count > 0);
        assert!(plan.compilation.global_effect_count > 0);
        assert!(plan.compilation.effect_pass_count > 0);
        validate_plan(&plan).expect("current generated effects are WGPU-capable");
    }

    #[test]
    fn capability_validation_rejects_a_kernel_when_the_backend_does_not_support_it() {
        let result = validate_kernel_capabilities_with(
            RenderBackendKind::Wgpu,
            [EffectKernel::Composite],
            |_| false,
        );
        assert_eq!(
            result.expect_err("unsupported kernel").code,
            "RENDER-UNSUPPORTED-KERNEL"
        );
        assert!(
            validate_kernel_capabilities(RenderBackendKind::Wgpu, [EffectKernel::Composite])
                .is_ok()
        );
    }

    #[test]
    fn unused_unsupported_kernel_does_not_disable_wgpu_validation() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/effects-ready.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("effects fixture validates");
        let plan =
            compile(&validated, CompileOptions::default()).expect("effects fixture compiles");
        let required = required_effect_kernels(&plan);
        assert!(!required.contains(EffectKernel::MotionBlur));
        validate_kernel_capabilities_with(RenderBackendKind::Wgpu, required.iter(), |kernel| {
            kernel != EffectKernel::MotionBlur
        })
        .expect("unrelated unsupported kernel is ignored");
    }

    #[test]
    fn required_unsupported_kernel_rejects_wgpu_but_not_cpu() {
        let required = [EffectKernel::MotionBlur];
        assert!(
            validate_kernel_capabilities_with(RenderBackendKind::Wgpu, required, |_| false,)
                .is_err()
        );
        validate_kernel_capabilities_with(RenderBackendKind::Cpu, required, |_| true)
            .expect("CPU supports the required test kernel");
    }
}
