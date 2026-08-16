//! Headless WGPU resource ownership for the render backend.

#![allow(
    clippy::result_large_err,
    reason = "WGPU preparation retains structured user-facing diagnostics"
)]

mod backend;
mod context;
mod diagnostics;
mod executor;
mod frame_plan;
mod parameters;
mod parity;
mod particles;
mod pipeline;
mod polling;
mod readback;
mod readback_state;
mod requirements;
mod resources;
mod runtime_error;
pub(crate) mod support;
mod texture_pool;

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixture_tests;
#[cfg(test)]
#[path = "tests/parity.rs"]
mod parity_tests;
#[cfg(test)]
#[path = "tests/readback.rs"]
mod readback_tests;
#[cfg(test)]
#[path = "tests/requirements.rs"]
mod requirements_tests;
#[cfg(test)]
#[path = "tests/shader.rs"]
mod shader_tests;

pub use backend::WgpuBackend;
pub use parity::{FrameDifference, PixelMismatch, compare_rgba};

/// Enumerate every adapter exposed by the requested WGPU instance backends.
/// This is intentionally separate from `probe`, which selects one adapter for
/// rendering and is therefore insufficient for validation discovery.
pub fn discover() -> Vec<crate::AdapterMetadata> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::default()
    });
    instance
        .enumerate_adapters(wgpu::Backends::all())
        .into_iter()
        .map(|adapter| {
            let info = adapter.get_info();
            crate::AdapterMetadata {
                adapter_name: info.name,
                device_type: format!("{:?}", info.device_type).to_lowercase(),
                graphics_backend: format!("{:?}", info.backend).to_lowercase(),
                driver_name: info.driver,
                driver_info: info.driver_info,
                vendor_id: info.vendor,
                device_id: info.device,
            }
        })
        .collect()
}

/// Checks the same adapter-selection policy used by the WGPU renderer without
/// retaining renderer resources. This is intentionally a capability check,
/// not render preparation.
pub fn probe() -> Result<crate::AdapterMetadata, crate::Diagnostic> {
    use crate::{Category, Diagnostic};
    use texture_pool::{WORKING_FORMAT, WORKING_TEXTURE_USAGE};

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: diagnostics::requested_backends(),
        ..wgpu::InstanceDescriptor::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: diagnostics::environment_present(
            "VESTRA_WGPU_FORCE_FALLBACK",
            "VIDEO_EDITOR_WGPU_FORCE_FALLBACK",
        ),
        compatible_surface: None,
    }))
    .ok_or_else(|| {
        Diagnostic::error(
            "WGPU-ADAPTER-NOT-FOUND",
            Category::Backend,
            "WGPU adapter request returned no compatible adapter",
            "",
        )
    })?;
    if !adapter
        .get_texture_format_features(WORKING_FORMAT)
        .allowed_usages
        .contains(WORKING_TEXTURE_USAGE)
    {
        return Err(Diagnostic::error(
            "WGPU-TEXTURE-FORMAT",
            Category::Backend,
            "WGPU adapter does not support Rgba8Unorm sampled, storage, and copy working textures",
            "",
        ));
    }
    let info = adapter.get_info();
    Ok(crate::AdapterMetadata {
        adapter_name: info.name,
        device_type: format!("{:?}", info.device_type).to_lowercase(),
        graphics_backend: format!("{:?}", info.backend).to_lowercase(),
        driver_name: info.driver,
        driver_info: info.driver_info,
        vendor_id: info.vendor,
        device_id: info.device,
    })
}

#[cfg(test)]
#[path = "tests/crops_gpu.rs"]
mod crop_gpu_tests;
#[cfg(test)]
#[path = "tests/gpu.rs"]
mod gpu;
#[cfg(test)]
#[path = "tests/parity_gpu.rs"]
mod parity_gpu_tests;
#[cfg(test)]
#[path = "tests/resources_gpu.rs"]
mod resource_gpu_tests;
