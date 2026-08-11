use crate::plan::{EffectOperation, RenderPlan, compiled_effect_pass_plan};

/// Renderer-owned implementation families for logical effect operations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum EffectKernel {
    ColourTransform,
    GaussianBlur,
    HighlightExtract,
    Composite,
    DirectionalBlur,
    ZoomBlur,
    ChromaticAberration,
    Vignette,
    ColorAdjust,
    MotionBlur,
}

/// The single renderer mapping from logical operations to backend kernels.
#[must_use]
pub(crate) const fn kernel_for_operation(operation: &EffectOperation) -> EffectKernel {
    match operation {
        EffectOperation::ApplyColourTransform { .. } => EffectKernel::ColourTransform,
        EffectOperation::GaussianHorizontal { .. } | EffectOperation::GaussianVertical { .. } => {
            EffectKernel::GaussianBlur
        }
        EffectOperation::HighlightExtract { .. } => EffectKernel::HighlightExtract,
        EffectOperation::Composite { .. } => EffectKernel::Composite,
        EffectOperation::DirectionalBlur { .. } => EffectKernel::DirectionalBlur,
        EffectOperation::ZoomBlur { .. } => EffectKernel::ZoomBlur,
        EffectOperation::ChromaticAberration { .. } => EffectKernel::ChromaticAberration,
        EffectOperation::Vignette { .. } => EffectKernel::Vignette,
        EffectOperation::ColorAdjust { .. } => EffectKernel::ColorAdjust,
        EffectOperation::MotionBlur { .. } => EffectKernel::MotionBlur,
    }
}

impl EffectKernel {
    pub(crate) const ALL: [Self; 10] = [
        Self::ColourTransform,
        Self::GaussianBlur,
        Self::HighlightExtract,
        Self::Composite,
        Self::DirectionalBlur,
        Self::ZoomBlur,
        Self::ChromaticAberration,
        Self::Vignette,
        Self::ColorAdjust,
        Self::MotionBlur,
    ];

    pub(crate) const fn supports_cpu(self) -> bool {
        true
    }
    pub(crate) const fn supports_wgpu(self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct EffectKernelSet(u16);

impl EffectKernelSet {
    pub(crate) const fn new() -> Self {
        Self(0)
    }

    pub(crate) fn insert(&mut self, kernel: EffectKernel) {
        self.0 |= 1 << kernel.index();
    }

    pub(crate) fn iter(self) -> impl Iterator<Item = EffectKernel> {
        EffectKernel::ALL
            .into_iter()
            .filter(move |kernel| self.0 & (1 << kernel.index()) != 0)
    }

    #[cfg(test)]
    pub(crate) fn contains(self, kernel: EffectKernel) -> bool {
        self.0 & (1 << kernel.index()) != 0
    }
}

impl EffectKernel {
    const fn index(self) -> u32 {
        match self {
            Self::ColourTransform => 0,
            Self::GaussianBlur => 1,
            Self::HighlightExtract => 2,
            Self::Composite => 3,
            Self::DirectionalBlur => 4,
            Self::ZoomBlur => 5,
            Self::ChromaticAberration => 6,
            Self::Vignette => 7,
            Self::ColorAdjust => 8,
            Self::MotionBlur => 9,
        }
    }
}

pub(crate) fn required_effect_kernels(plan: &RenderPlan) -> EffectKernelSet {
    let mut required = EffectKernelSet::new();
    for effect in plan
        .layers
        .iter()
        .flat_map(|layer| layer.effects.iter())
        .chain(plan.post_effects.iter())
    {
        for pass in compiled_effect_pass_plan(&effect.effect).iter() {
            required.insert(kernel_for_operation(&pass.operation));
        }
    }
    required
}

pub(crate) fn validate_plan_capabilities(
    plan: &RenderPlan,
    backend: crate::backend::RenderBackendKind,
) -> Result<(), crate::Diagnostic> {
    validate_required_kernels(backend, required_effect_kernels(plan).iter())
}

pub(crate) fn validate_required_kernels(
    backend: crate::backend::RenderBackendKind,
    kernels: impl IntoIterator<Item = EffectKernel>,
) -> Result<(), crate::Diagnostic> {
    for kernel in kernels {
        if !supports_kernel(backend, kernel) {
            return Err(crate::Diagnostic::error(
                "WGPU-UNSUPPORTED-KERNEL",
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

#[must_use]
pub(crate) const fn supports_kernel(
    backend: crate::backend::RenderBackendKind,
    kernel: EffectKernel,
) -> bool {
    match backend {
        crate::backend::RenderBackendKind::Cpu => kernel.supports_cpu(),
        crate::backend::RenderBackendKind::Wgpu => kernel.supports_wgpu(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{CompositeMode, EffectOperation};

    #[test]
    fn gaussian_directions_share_a_kernel() {
        assert_eq!(
            kernel_for_operation(&EffectOperation::GaussianHorizontal { radius: 1.0 }),
            EffectKernel::GaussianBlur
        );
        assert_eq!(
            kernel_for_operation(&EffectOperation::GaussianVertical { radius: 1.0 }),
            EffectKernel::GaussianBlur
        );
    }

    #[test]
    fn composite_modes_share_a_kernel() {
        for mode in [CompositeMode::Additive, CompositeMode::Unsharp] {
            assert_eq!(
                kernel_for_operation(&EffectOperation::Composite { mode, amount: 1.0 }),
                EffectKernel::Composite
            );
        }
    }

    #[test]
    fn every_declared_kernel_is_supported_by_both_backends() {
        assert!(
            EffectKernel::ALL
                .into_iter()
                .all(EffectKernel::supports_cpu)
        );
        assert!(
            EffectKernel::ALL
                .into_iter()
                .all(EffectKernel::supports_wgpu)
        );
    }

    #[test]
    fn capability_queries_are_backend_typed() {
        assert!(supports_kernel(
            crate::backend::RenderBackendKind::Cpu,
            EffectKernel::Composite
        ));
        assert!(supports_kernel(
            crate::backend::RenderBackendKind::Wgpu,
            EffectKernel::Composite
        ));
    }
}
