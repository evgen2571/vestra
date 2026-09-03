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
    // Rust drops fields in declaration order. Drop the queue before the
    // device so wgpu-core observes an empty device queue, then release the
    // device's parent adapter and instance.
    pub(super) queue: wgpu::Queue,
    pub(super) device: wgpu::Device,
    pub(super) _adapter: wgpu::Adapter,
    pub(super) adapter_metadata: AdapterMetadata,
    pub(super) adapter_limits: wgpu::Limits,
    pub(super) adapter_request: Duration,
    pub(super) device_request: Duration,
    pub(super) runtime_errors: RuntimeErrorState,
}

impl Drop for GpuContext {
    fn drop(&mut self) {
        // This runs after WgpuBackend's resource fields because the context
        // is declared last. Drain resource destruction while the device is
        // still alive, immediately before queue/device teardown.
        self.device.poll(wgpu::Maintain::Wait);
    }
}

impl GpuContext {
    pub(super) fn create(
        plan: &RenderPlan,
        requirements: GpuRequirements,
    ) -> Result<Self, Diagnostic> {
        let instance = super::diagnostics::instance_for_backends(requested_backends());
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
            let info = adapter.get_info();
            tracing::debug!(
                target: "vestra.render.wgpu",
                adapter = %info.name,
                device_type = ?info.device_type,
                reason = "working texture format does not support required usages",
                error_code = "WGPU-TEXTURE-FORMAT",
                "WGPU adapter rejected"
            );
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
        let performance_class = adapter_metadata.performance_class();
        tracing::debug!(
            target: "vestra.render.wgpu",
            graphics_backend = %adapter_metadata.graphics_backend,
            adapter = %adapter_metadata.adapter_name,
            device_type = %adapter_metadata.device_type,
            driver_name = %adapter_metadata.driver_name,
            driver_info = %adapter_metadata.driver_info,
            vendor_id = adapter_metadata.vendor_id,
            device_id = adapter_metadata.device_id,
            hardware = performance_class.is_proven_hardware(),
            classification = performance_class.as_str(),
            "WGPU adapter candidate selected"
        );
        if performance_class.is_software() {
            tracing::debug!(
                target: "vestra.render.wgpu",
                graphics_backend = %adapter_metadata.graphics_backend,
                adapter = %adapter_metadata.adapter_name,
                device_type = %adapter_metadata.device_type,
                hardware = false,
                classification = performance_class.as_str(),
                "software WGPU adapter candidate selected"
            );
        }
        let adapter_limits = adapter.limits();
        let requested_limits = requirements.requested_device_limits(plan, &adapter_limits)?;
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
        tracing::debug!(
            target: "vestra.render.wgpu",
            graphics_backend = %adapter_metadata.graphics_backend,
            adapter = %adapter_metadata.adapter_name,
            device_type = %adapter_metadata.device_type,
            elapsed_ms = crate::trace_milliseconds(device_request),
            "GPU device initialized"
        );
        requirements.validate(&device.limits(), plan)?;
        let runtime_errors = RuntimeErrorState::install(&device);
        // Report the adapter as selected only after device creation and
        // plan-specific limit validation succeed. A discovered candidate is
        // not yet a usable render context.
        tracing::info!(
            target: "vestra.render.wgpu",
            actual_backend = "wgpu",
            graphics_backend = %adapter_metadata.graphics_backend,
            adapter = %adapter_metadata.adapter_name,
            device_type = %adapter_metadata.device_type,
            driver_name = %adapter_metadata.driver_name,
            driver_info = %adapter_metadata.driver_info,
            vendor_id = adapter_metadata.vendor_id,
            device_id = adapter_metadata.device_id,
            hardware = performance_class.is_proven_hardware(),
            classification = performance_class.as_str(),
            "WGPU adapter selected"
        );
        Ok(Self {
            queue,
            device,
            _adapter: adapter,
            adapter_metadata,
            adapter_limits,
            adapter_request,
            device_request,
            runtime_errors,
        })
    }
}
