//! Error-scope handling and environment-derived WGPU backend selection.

use std::sync::OnceLock;

use crate::{Category, Diagnostic};

static ALL_INSTANCE: OnceLock<wgpu::Instance> = OnceLock::new();
static GL_INSTANCE: OnceLock<wgpu::Instance> = OnceLock::new();
static VULKAN_INSTANCE: OnceLock<wgpu::Instance> = OnceLock::new();
static DX12_INSTANCE: OnceLock<wgpu::Instance> = OnceLock::new();
static METAL_INSTANCE: OnceLock<wgpu::Instance> = OnceLock::new();
static BROWSER_WEBGPU_INSTANCE: OnceLock<wgpu::Instance> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstanceSlot {
    All,
    Gl,
    Vulkan,
    Dx12,
    Metal,
    BrowserWebGpu,
    Dedicated,
}

fn instance_slot(backends: wgpu::Backends) -> InstanceSlot {
    match backends {
        value if value == wgpu::Backends::all() => InstanceSlot::All,
        value if value == wgpu::Backends::GL => InstanceSlot::Gl,
        value if value == wgpu::Backends::VULKAN => InstanceSlot::Vulkan,
        value if value == wgpu::Backends::DX12 => InstanceSlot::Dx12,
        value if value == wgpu::Backends::METAL => InstanceSlot::Metal,
        value if value == wgpu::Backends::BROWSER_WEBGPU => InstanceSlot::BrowserWebGpu,
        _ => InstanceSlot::Dedicated,
    }
}

/// Keep WGPU root instances process-owned: WSL Mesa/D3D12 GL can crash during
/// EGL instance destruction after tests. Renderer resources, devices, queues,
/// and decoder sessions still have ordinary deterministic ownership.
pub(super) fn instance_for_backends(backends: wgpu::Backends) -> &'static wgpu::Instance {
    let descriptor = || wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::default()
    };
    match instance_slot(backends) {
        InstanceSlot::All => ALL_INSTANCE.get_or_init(|| wgpu::Instance::new(descriptor())),
        InstanceSlot::Gl => GL_INSTANCE.get_or_init(|| wgpu::Instance::new(descriptor())),
        InstanceSlot::Vulkan => VULKAN_INSTANCE.get_or_init(|| wgpu::Instance::new(descriptor())),
        InstanceSlot::Dx12 => DX12_INSTANCE.get_or_init(|| wgpu::Instance::new(descriptor())),
        InstanceSlot::Metal => METAL_INSTANCE.get_or_init(|| wgpu::Instance::new(descriptor())),
        InstanceSlot::BrowserWebGpu => {
            BROWSER_WEBGPU_INSTANCE.get_or_init(|| wgpu::Instance::new(descriptor()))
        }
        // Combined masks outside `all()` must not reuse an incompatible root.
        InstanceSlot::Dedicated => Box::leak(Box::new(wgpu::Instance::new(descriptor()))),
    }
}

pub(super) fn finish_error_scopes(device: &wgpu::Device, code: &str) -> Result<(), Diagnostic> {
    let internal_error = pollster::block_on(device.pop_error_scope());
    let validation_error = pollster::block_on(device.pop_error_scope());
    if let Some(error) = internal_error {
        return Err(diagnostic(code, "internal", error));
    }
    if let Some(error) = validation_error {
        return Err(diagnostic(code, "validation", error));
    }
    Ok(())
}

pub(super) fn requested_backends() -> wgpu::Backends {
    match std::env::var("VESTRA_WGPU_BACKEND")
        .ok()
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("vulkan") => wgpu::Backends::VULKAN,
        Some("gl") | Some("gles") => wgpu::Backends::GL,
        Some("metal") => wgpu::Backends::METAL,
        Some("dx12") => wgpu::Backends::DX12,
        Some("browser_webgpu") => wgpu::Backends::BROWSER_WEBGPU,
        _ => wgpu::Backends::all(),
    }
}

pub(super) fn diagnostic(code: &str, stage: &str, error: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(
        code,
        Category::Backend,
        format!("WGPU {stage} failed: {error}"),
        "",
    )
}

#[cfg(test)]
mod tests {
    use super::{InstanceSlot, instance_slot};

    #[test]
    fn backend_masks_use_distinct_slots() {
        assert_eq!(instance_slot(wgpu::Backends::GL), InstanceSlot::Gl);
        assert_eq!(instance_slot(wgpu::Backends::VULKAN), InstanceSlot::Vulkan);
        assert_eq!(instance_slot(wgpu::Backends::all()), InstanceSlot::All);
        assert_eq!(instance_slot(wgpu::Backends::DX12), InstanceSlot::Dx12);
        assert_eq!(instance_slot(wgpu::Backends::METAL), InstanceSlot::Metal);
        assert_eq!(
            instance_slot(wgpu::Backends::BROWSER_WEBGPU),
            InstanceSlot::BrowserWebGpu
        );
    }

    #[test]
    fn combined_masks_get_dedicated_instances() {
        assert_eq!(
            instance_slot(wgpu::Backends::GL | wgpu::Backends::VULKAN),
            InstanceSlot::Dedicated
        );
    }

    #[test]
    fn initialization_order_cannot_narrow_the_all_instance() {
        let gl = super::instance_for_backends(wgpu::Backends::GL);
        let all = super::instance_for_backends(wgpu::Backends::all());
        assert!(!std::ptr::eq(gl, all));
        assert!(std::ptr::eq(
            all,
            super::instance_for_backends(wgpu::Backends::all())
        ));
    }
}
