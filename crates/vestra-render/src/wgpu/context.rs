//! Adapter discovery and device creation for a prepared WGPU backend.

use std::time::{Duration, Instant};

use crate::{Category, Diagnostic, plan::RenderPlan, render::AdapterMetadata};

use super::{
    diagnostics::{diagnostic, environment_present, requested_backends},
    requirements::GpuRequirements,
    runtime_error::RuntimeErrorState,
    texture_pool::{WORKING_FORMAT, WORKING_TEXTURE_USAGE},
};

pub(super) struct GpuContext {
    pub(super) _instance: wgpu::Instance,
    pub(super) _adapter: wgpu::Adapter,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) adapter_metadata: AdapterMetadata,
    pub(super) adapter_limits: wgpu::Limits,
    pub(super) adapter_request: Duration,
    pub(super) device_request: Duration,
    pub(super) runtime_errors: RuntimeErrorState,
}

impl GpuContext {
    pub(super) fn create(
        plan: &RenderPlan,
        requirements: GpuRequirements,
    ) -> Result<Self, Diagnostic> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: requested_backends(),
            ..wgpu::InstanceDescriptor::default()
        });
        let adapter_request_started = Instant::now();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: environment_present(
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
        let adapter_request = adapter_request_started.elapsed();
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
        let adapter_metadata = AdapterMetadata {
            adapter_name: info.name,
            device_type: format!("{:?}", info.device_type).to_lowercase(),
            graphics_backend: format!("{:?}", info.backend).to_lowercase(),
            driver_name: info.driver,
            driver_info: info.driver_info,
            vendor_id: info.vendor,
            device_id: info.device,
        };
        let adapter_limits = adapter.limits();
        requirements.validate(&adapter_limits, plan)?;
        let requested_limits = requirements.requested_device_limits(plan)?;
        let device_request_started = Instant::now();
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("vestra headless renderer"),
                required_features: wgpu::Features::empty(),
                required_limits: requested_limits,
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|error| diagnostic("WGPU-DEVICE-REQUEST", "device_request", error))?;
        let device_request = device_request_started.elapsed();
        requirements.validate(&device.limits(), plan)?;
        let runtime_errors = RuntimeErrorState::install(&device);
        Ok(Self {
            _instance: instance,
            _adapter: adapter,
            device,
            queue,
            adapter_metadata,
            adapter_limits,
            adapter_request,
            device_request,
            runtime_errors,
        })
    }
}
