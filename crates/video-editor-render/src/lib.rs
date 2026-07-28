//! Internal frame renderer for `video-editor`.
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

pub use video_editor_core::{Category, Diagnostic, Severity, animation, domain};
pub mod plan {
    #[cfg(not(test))]
    pub use video_editor_core::plan::*;
    #[cfg(test)]
    pub use video_editor_core::plan::{self, *};

    /// Test compatibility with the root's former planning facade. Production
    /// code receives plans from root orchestration and does not compile them.
    #[cfg(test)]
    pub fn compile(
        input: &PlanCompileInput<'_>,
        options: CompileOptions,
    ) -> Result<RenderPlan, crate::Diagnostic> {
        self::compile_input(*input, options)
    }

    #[cfg(test)]
    use video_editor_core::plan::compile as compile_input;
}
/// Temporary internal facade retained while the root compatibility imports are
/// removed. It exposes only core project values used by pixel production.
pub mod project {
    pub use video_editor_core::project::*;

    #[cfg(test)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct ValidationOptions {
        pub check_backend: bool,
        /// Keeps migrated root test literals source-compatible.
        pub root_fixture_compat: bool,
    }

    /// Test-only fixture loader. Production path resolution and environment
    /// preflight remain in the root package.
    #[cfg(test)]
    pub fn load_and_validate(
        path: &std::path::Path,
        _options: &ValidationOptions,
    ) -> Result<crate::plan::PlanCompileInput<'static>, crate::Diagnostic> {
        use std::collections::BTreeMap;

        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path)
        };
        let bytes = std::fs::read(&path).map_err(|error| {
            crate::Diagnostic::error(
                "MVP-PROJECT-READ",
                crate::Category::Project,
                format!("cannot read project: {error}"),
                "",
            )
        })?;
        let project = Box::leak(Box::new(
            serde_json::from_slice::<Project>(&bytes).map_err(|error| {
                crate::Diagnostic::error(
                    "MVP-PROJECT-SHAPE",
                    crate::Category::Project,
                    format!("project does not match the canonical format: {error}"),
                    "",
                )
            })?,
        ));
        let root = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let asset_paths = Box::leak(Box::new(
            project
                .assets
                .iter()
                .map(|asset| (asset.id.clone(), root.join(&asset.source)))
                .collect::<BTreeMap<_, _>>(),
        ));
        let audio_durations = Box::leak(Box::new(BTreeMap::new()));
        let duration = project
            .visual
            .clips
            .iter()
            .map(|clip| clip.start + clip.duration)
            .fold(0.0_f64, f64::max);
        let frame_rate = project.output.frame_rate.rational().map_err(|message| {
            crate::Diagnostic::error("MVP-OUTPUT-FPS", crate::Category::Semantic, message, "")
        })?;
        let frame_count = video_editor_core::timeline::frame_count(
            video_editor_core::timeline::seconds_to_nanos(duration).unwrap_or(0),
            frame_rate.0,
            frame_rate.1,
        );
        Ok(crate::plan::PlanCompileInput::new(
            project,
            video_editor_core::validation::ResourceLimits::default(),
            Box::leak(path.into_boxed_path()),
            asset_paths,
            audio_durations,
            duration,
            frame_rate,
            frame_count,
            &[],
        ))
    }
}
#[cfg(test)]
pub use video_editor_core::timeline;

mod backend;
pub mod blend;
mod cache;
#[cfg(feature = "cpu")]
pub mod cpu;
pub mod decoded;
pub mod effects;
pub mod geometry;
pub mod metrics;
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

/// Internal import compatibility for renderer modules moved from the root.
/// This disappears with the temporary root facade in the SDK extraction.
pub mod render {
    pub use crate::CpuBackend;
    #[cfg(feature = "wgpu")]
    pub use crate::WgpuBackend;
    pub use crate::blend;
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
    plan: &plan::RenderPlan,
    decoded: &std::sync::Arc<DecodedAssets>,
) -> Result<(Box<dyn RenderBackend>, Option<BackendFallback>), Diagnostic> {
    #[cfg(feature = "wgpu")]
    if let Err(error) = wgpu::support::validate_plan(plan) {
        return match preference {
            RenderBackendPreference::Wgpu => Err(error),
            RenderBackendPreference::Auto => Ok((
                Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
                Some(BackendFallback {
                    code: error.code,
                    stage: "effect_capability".to_owned(),
                    message: error.message,
                }),
            )),
            RenderBackendPreference::Cpu => Ok((
                Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
                None,
            )),
        };
    }
    match preference {
        RenderBackendPreference::Cpu => Ok((
            Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
            None,
        )),
        #[cfg(feature = "wgpu")]
        RenderBackendPreference::Wgpu => Ok((
            Box::new(WgpuBackend::new(plan, std::sync::Arc::clone(decoded))?),
            None,
        )),
        #[cfg(not(feature = "wgpu"))]
        RenderBackendPreference::Wgpu => Err(Diagnostic::error(
            "MVP-WGPU-UNAVAILABLE",
            Category::Backend,
            "WGPU support is not enabled in this build",
            "",
        )),
        #[cfg(feature = "wgpu")]
        RenderBackendPreference::Auto => {
            match WgpuBackend::new(plan, std::sync::Arc::clone(decoded)) {
                Ok(backend) => Ok((Box::new(backend), None)),
                Err(error) => Ok((
                    Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
                    Some(BackendFallback {
                        code: error.code,
                        stage: "wgpu_preparation".to_owned(),
                        message: error.message,
                    }),
                )),
            }
        }
        #[cfg(not(feature = "wgpu"))]
        RenderBackendPreference::Auto => Ok((
            Box::new(CpuBackend::new(plan, std::sync::Arc::clone(decoded))),
            None,
        )),
    }
}
