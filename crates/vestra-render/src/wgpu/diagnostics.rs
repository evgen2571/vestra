//! Error-scope handling and environment-derived WGPU backend selection.

use std::sync::OnceLock;

use crate::{Category, Diagnostic};

static ALL_INSTANCE: OnceLock<wgpu::Instance> = OnceLock::new();
static GL_INSTANCE: OnceLock<wgpu::Instance> = OnceLock::new();
static VULKAN_INSTANCE: OnceLock<wgpu::Instance> = OnceLock::new();

/// WSL's Mesa/D3D12 GL path is not safe to tear down while the process still
/// owns WGPU resource state. Keep the small WGPU root instance process-owned;
/// renderers and devices remain ordinary owned values and are still dropped
/// deterministically before process exit.
pub(super) fn instance_for_backends(backends: wgpu::Backends) -> &'static wgpu::Instance {
    let slot = if backends == wgpu::Backends::GL {
        &GL_INSTANCE
    } else if backends == wgpu::Backends::VULKAN {
        &VULKAN_INSTANCE
    } else {
        &ALL_INSTANCE
    };
    slot.get_or_init(|| {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::default()
        })
    })
}

pub(super) fn environment_value(canonical: &str, legacy: &str) -> Option<String> {
    std::env::var(canonical)
        .ok()
        .or_else(|| std::env::var(legacy).ok())
}

pub(super) fn environment_present(canonical: &str, legacy: &str) -> bool {
    std::env::var_os(canonical).is_some() || std::env::var_os(legacy).is_some()
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
    match environment_value("VESTRA_WGPU_BACKEND", "VIDEO_EDITOR_WGPU_BACKEND")
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
