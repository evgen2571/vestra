//! Internal frame renderer for `vestra`.
//!
//! This crate turns core-owned evaluated frames into owned RGBA frames. It owns
//! CPU and WGPU backends, decoded visual assets, renderer geometry, effect
//! execution, resource management, and staged completion polling. It does not
//! probe or encode media, publish files, or coordinate the application or CLI.
//! Its workspace API is intentionally unstable.

#![allow(
    clippy::result_large_err,
    reason = "renderer diagnostics retain structured user-facing context"
)]

pub use vestra_core::{Category, Diagnostic, Severity, animation, domain};
mod kernel;

pub(crate) fn trace_milliseconds(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
/// Core-owned planning types exposed through the renderer's established path.
pub mod plan {
    pub use vestra_core::plan::*;
}

/// Internal project values used by renderer tests and pixel production.
/// Production path resolution and environment preflight belong to the SDK.
pub mod project {
    pub use vestra_core::project::*;
}
#[cfg(test)]
mod test_support;
#[cfg(test)]
pub use vestra_core::timeline;

mod backend;
pub mod blend;
mod cache;
#[cfg(feature = "cpu")]
pub mod cpu;
pub mod decoded;
pub mod effects;
pub mod geometry;
pub mod metrics;
#[path = "cpu/shapes.rs"]
pub(crate) mod shape_raster;
pub(crate) mod text;
pub mod video;
#[cfg(feature = "wgpu")]
mod wgpu;

pub use backend::{
    AdapterMetadata, AdapterPerformanceClass, CompletedFrame, PollMode, RenderBackend,
    RenderBackendKind,
};
pub use cache::{ByteLruCache, CacheStats};
#[cfg(feature = "cpu")]
pub use cpu::backend::CpuBackend;
pub use decoded::DecodedAssets;
pub use metrics::{PreparationStats, PreparationTimings, StagedMetrics};
pub use video::{VideoDecoderFactory, VideoDecoderMetrics, VideoDecoderSession, VideoFrame};

#[cfg(feature = "wgpu")]
pub fn discover_wgpu_adapters() -> Vec<AdapterMetadata> {
    wgpu::discover()
}
#[cfg(feature = "wgpu")]
pub use wgpu::{FrameDifference, PixelMismatch, WgpuBackend, compare_rgba};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderBackendPreference {
    Auto,
    Cpu,
    Wgpu,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendFallback {
    pub code: String,
    pub stage: String,
    pub message: String,
}

/// Renderer-owned readiness result. It carries stable facts, never raw WGPU
/// handles, so callers use the renderer's backend policy without duplicating it.
#[derive(Clone, Debug)]
pub struct BackendProbe {
    pub selected: Option<RenderBackendPreference>,
    pub fallback: Option<BackendFallback>,
    pub adapter: Option<AdapterMetadata>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn probe_backend(preference: RenderBackendPreference) -> BackendProbe {
    match preference {
        RenderBackendPreference::Cpu => BackendProbe {
            selected: cpu_probe(),
            fallback: None,
            adapter: None,
            diagnostics: Vec::new(),
        },
        RenderBackendPreference::Wgpu => match wgpu_probe() {
            Ok(adapter) => BackendProbe {
                selected: Some(RenderBackendPreference::Wgpu),
                fallback: None,
                adapter: Some(adapter),
                diagnostics: Vec::new(),
            },
            Err(diagnostic) => BackendProbe {
                selected: None,
                fallback: None,
                adapter: None,
                diagnostics: vec![diagnostic],
            },
        },
        RenderBackendPreference::Auto => match wgpu_probe() {
            Ok(adapter) => BackendProbe {
                selected: Some(RenderBackendPreference::Wgpu),
                fallback: None,
                adapter: Some(adapter),
                diagnostics: Vec::new(),
            },
            Err(diagnostic) => BackendProbe {
                selected: cpu_probe(),
                fallback: cpu_probe().map(|_| BackendFallback {
                    code: diagnostic.code.clone(),
                    stage: "wgpu_probe".to_owned(),
                    message: diagnostic.message.clone(),
                }),
                adapter: None,
                diagnostics: if cpu_probe().is_some() {
                    vec![Diagnostic::warning(
                        "VESTRA-WGPU-FALLBACK",
                        format!("WGPU fallback to CPU: {}", diagnostic.message),
                        "",
                    )]
                } else {
                    vec![diagnostic]
                },
            },
        },
    }
}

#[cfg(feature = "cpu")]
fn cpu_probe() -> Option<RenderBackendPreference> {
    Some(RenderBackendPreference::Cpu)
}
#[cfg(not(feature = "cpu"))]
fn cpu_probe() -> Option<RenderBackendPreference> {
    None
}

#[cfg(feature = "wgpu")]
fn wgpu_probe() -> Result<AdapterMetadata, Diagnostic> {
    wgpu::probe()
}
#[cfg(not(feature = "wgpu"))]
fn wgpu_probe() -> Result<AdapterMetadata, Diagnostic> {
    Err(Diagnostic::error(
        "VESTRA-WGPU-UNAVAILABLE",
        Category::Backend,
        "WGPU support is not enabled in this build",
        "",
    ))
}

/// Internal renderer imports shared by renderer modules.
pub mod render {
    #[cfg(feature = "cpu")]
    pub use crate::CpuBackend;
    #[cfg(feature = "wgpu")]
    pub use crate::WgpuBackend;
    pub use crate::blend;
    #[cfg(feature = "cpu")]
    pub use crate::cpu;
    pub use crate::decoded;
    pub use crate::effects;
    pub use crate::geometry;
    pub use crate::metrics;
    pub use crate::{
        AdapterMetadata, ByteLruCache, CompletedFrame, DecodedAssets, PollMode, RenderBackend,
        RenderBackendKind, StagedMetrics,
    };
}

#[expect(
    clippy::result_large_err,
    reason = "backend selection preserves diagnostics"
)]
pub fn create_backend(
    preference: RenderBackendPreference,
    plan: &vestra_core::plan::RenderPlan,
    decoded: &std::sync::Arc<DecodedAssets>,
) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic> {
    #[cfg(feature = "cpu")]
    if matches!(preference, RenderBackendPreference::Cpu) {
        kernel::validate_plan_capabilities(plan, RenderBackendKind::Cpu)?;
    }

    #[cfg(all(feature = "cpu", not(feature = "wgpu")))]
    if matches!(preference, RenderBackendPreference::Auto) {
        kernel::validate_plan_capabilities(plan, RenderBackendKind::Cpu)?;
    }

    #[cfg(feature = "wgpu")]
    if let Err(error) = wgpu::support::validate_plan(plan) {
        return match preference {
            RenderBackendPreference::Wgpu => Err(error),
            #[cfg(feature = "cpu")]
            RenderBackendPreference::Auto => {
                kernel::validate_plan_capabilities(plan, RenderBackendKind::Cpu)?;
                Ok((
                    Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
                    Some(BackendFallback {
                        code: error.code,
                        stage: "effect_capability".to_owned(),
                        message: error.message,
                    }),
                ))
            }
            #[cfg(feature = "cpu")]
            RenderBackendPreference::Cpu => Ok((
                Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
                None,
            )),
            #[cfg(not(feature = "cpu"))]
            RenderBackendPreference::Auto | RenderBackendPreference::Cpu => Err(Diagnostic::error(
                "VESTRA-CPU-UNAVAILABLE",
                Category::Backend,
                "CPU support is not enabled in this build",
                "",
            )),
        };
    }
    match preference {
        #[cfg(feature = "cpu")]
        RenderBackendPreference::Cpu => Ok((
            Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
            None,
        )),
        #[cfg(not(feature = "cpu"))]
        RenderBackendPreference::Cpu => Err(Diagnostic::error(
            "VESTRA-CPU-UNAVAILABLE",
            Category::Backend,
            "CPU support is not enabled in this build",
            "",
        )),
        #[cfg(feature = "wgpu")]
        RenderBackendPreference::Wgpu => Ok((
            Box::new(WgpuBackend::new(plan, std::sync::Arc::clone(decoded))?),
            None,
        )),
        #[cfg(not(feature = "wgpu"))]
        RenderBackendPreference::Wgpu => Err(Diagnostic::error(
            "VESTRA-WGPU-UNAVAILABLE",
            Category::Backend,
            "WGPU support is not enabled in this build",
            "",
        )),
        #[cfg(all(feature = "wgpu", feature = "cpu"))]
        RenderBackendPreference::Auto => {
            match WgpuBackend::new(plan, std::sync::Arc::clone(decoded)) {
                Ok(backend) => Ok((Box::new(backend), None)),
                Err(error) => {
                    kernel::validate_plan_capabilities(plan, RenderBackendKind::Cpu)?;
                    Ok((
                        Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
                        Some(BackendFallback {
                            code: error.code,
                            stage: "wgpu_preparation".to_owned(),
                            message: error.message,
                        }),
                    ))
                }
            }
        }
        #[cfg(all(feature = "wgpu", not(feature = "cpu")))]
        RenderBackendPreference::Auto => Ok((
            Box::new(WgpuBackend::new(plan, std::sync::Arc::clone(decoded))?),
            None,
        )),
        #[cfg(all(not(feature = "wgpu"), feature = "cpu"))]
        RenderBackendPreference::Auto => Ok((
            Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
            None,
        )),
        #[cfg(all(not(feature = "wgpu"), not(feature = "cpu")))]
        RenderBackendPreference::Auto => Err(Diagnostic::error(
            "VESTRA-BACKEND-UNAVAILABLE",
            Category::Backend,
            "no renderer backend is enabled in this build",
            "",
        )),
    }
}
