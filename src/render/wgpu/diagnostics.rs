//! Error-scope handling and environment-derived WGPU backend selection.

use crate::{Category, Diagnostic};

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
    match std::env::var("VIDEO_EDITOR_WGPU_BACKEND")
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
