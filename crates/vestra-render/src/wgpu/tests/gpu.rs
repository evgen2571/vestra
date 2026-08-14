//! Shared adapter setup for WGPU tests.

use std::sync::Arc;

use super::WgpuBackend;
use crate::{AdapterPerformanceClass, render::RenderBackend};

pub(super) fn hardware_wgpu_backend_or_skip(
    plan: &crate::plan::RenderPlan,
    decoded: Arc<crate::DecodedAssets>,
) -> Option<WgpuBackend> {
    let backend = wgpu_backend_or_skip(plan, decoded)?;
    let adapter = backend.adapter().expect("WGPU backend reports its adapter");
    match hardware_conformance_policy(adapter.performance_class(), require_hardware_wgpu()) {
        Ok(true) => Some(backend),
        Ok(false) => {
            eprintln!(
                "WGPU_RUNTIME_SKIPPED reason=software-adapter hardware-conformance adapter={} backend={} device_type={}",
                adapter.adapter_name, adapter.graphics_backend, adapter.device_type
            );
            None
        }
        Err(message) => panic!(
            "{message}: adapter={} backend={} device_type={}",
            adapter.adapter_name, adapter.graphics_backend, adapter.device_type
        ),
    }
}

pub(super) fn wgpu_backend_or_skip(
    plan: &crate::plan::RenderPlan,
    decoded: Arc<crate::DecodedAssets>,
) -> Option<WgpuBackend> {
    match WgpuBackend::new(plan, decoded) {
        Ok(backend) => {
            let adapter = backend.adapter().expect("WGPU backend reports its adapter");
            eprintln!(
                "WGPU_RUNTIME_EXECUTED adapter={} backend={} device_type={} vendor={} device={}",
                adapter.adapter_name,
                adapter.graphics_backend,
                adapter.device_type,
                adapter.vendor_id,
                adapter.device_id,
            );
            if strict_wgpu_required()
                && let Some(requested) = std::env::var_os("VESTRA_WGPU_BACKEND")
                && requested.eq_ignore_ascii_case("vulkan")
            {
                assert_eq!(
                    adapter.graphics_backend, "vulkan",
                    "strict Vulkan verification selected a different WGPU backend"
                );
            }
            Some(backend)
        }
        Err(error) if strict_wgpu_required() => {
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
            let adapter = backend.adapter().expect("WGPU backend reports its adapter");
            eprintln!(
                "WGPU_RUNTIME_EXECUTED adapter={} backend={} device_type={} vendor={} device={} depth={depth}",
                adapter.adapter_name,
                adapter.graphics_backend,
                adapter.device_type,
                adapter.vendor_id,
                adapter.device_id,
            );
            if strict_wgpu_required()
                && let Some(requested) = std::env::var_os("VESTRA_WGPU_BACKEND")
                && requested.eq_ignore_ascii_case("vulkan")
            {
                assert_eq!(
                    adapter.graphics_backend, "vulkan",
                    "strict Vulkan verification selected a different WGPU backend"
                );
            }
            Some(backend)
        }
        Err(error) if strict_wgpu_required() => {
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

fn require_hardware_wgpu() -> bool {
    std::env::var_os("VESTRA_REQUIRE_HARDWARE_WGPU").is_some()
}

fn strict_wgpu_required() -> bool {
    std::env::var_os("VESTRA_REQUIRE_WGPU").is_some() || require_hardware_wgpu()
}

fn hardware_conformance_policy(
    performance_class: AdapterPerformanceClass,
    require_hardware: bool,
) -> Result<bool, &'static str> {
    if performance_class.is_software() {
        if require_hardware {
            Err(
                "hardware WGPU conformance was explicitly required, but the selected adapter is software",
            )
        } else {
            Ok(false)
        }
    } else {
        Ok(true)
    }
}

fn is_environment_unavailable(error: &crate::Diagnostic) -> bool {
    matches!(
        error.code.as_str(),
        "WGPU-ADAPTER-NOT-FOUND" | "WGPU-NO-COMPATIBLE-ADAPTER"
    )
}

#[cfg(test)]
mod tests {
    use super::hardware_conformance_policy;
    use crate::AdapterPerformanceClass;

    #[test]
    fn software_adapter_skips_hardware_conformance_by_default() {
        assert_eq!(
            hardware_conformance_policy(AdapterPerformanceClass::Software, false),
            Ok(false)
        );
    }

    #[test]
    fn software_adapter_fails_when_hardware_conformance_is_required() {
        assert!(hardware_conformance_policy(AdapterPerformanceClass::Software, true).is_err());
    }

    #[test]
    fn hardware_adapter_runs_hardware_conformance_in_both_modes() {
        assert_eq!(
            hardware_conformance_policy(AdapterPerformanceClass::DiscreteGpu, false),
            Ok(true)
        );
        assert_eq!(
            hardware_conformance_policy(AdapterPerformanceClass::DiscreteGpu, true),
            Ok(true)
        );
    }
}
