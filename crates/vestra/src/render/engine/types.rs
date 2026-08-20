//! Public render-engine options, events, summaries, and failure DTOs.

use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use serde::Serialize;

use crate::{
    Diagnostic,
    render::{AdapterMetadata, PreparationStats, RenderBackendKind},
};

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub output_override: Option<PathBuf>,
    pub overwrite: bool,
    pub cancelled: Arc<AtomicBool>,
    /// Test-only factory selection; production operations cannot select a backend.
    #[cfg(test)]
    pub backend_preference: RenderBackendPreference,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderBackendPreference {
    #[default]
    Auto,
    Cpu,
    Wgpu,
}

impl RenderBackendPreference {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Wgpu => "wgpu",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BackendFallback {
    pub code: String,
    pub stage: String,
    pub message: String,
}

pub(crate) fn backend_fallback_warning(fallback: &BackendFallback) -> Diagnostic {
    Diagnostic::warning(
        "VESTRA-WGPU-FALLBACK",
        format!("WGPU fallback to CPU: {}", fallback.message),
        "",
    )
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RenderTimings {
    /// JSON parsing spent while constructing `Project`; zero for `from_value`.
    pub project_parse_ms: u128,
    /// All work inside `Editor::render`, from semantic validation through cleanup.
    /// It excludes `Project` construction and is not the sum of stage fields.
    pub operation_total_ms: u128,
    /// Pure semantic validation used by this render operation.
    pub semantic_validation_ms: u128,
    /// Environment and target readiness checks used by this render operation.
    pub preflight_ms: u128,
    /// Compilation time when compilation succeeded. On a render-stage failure,
    /// later renderer fields contain only timings known before the failure.
    pub plan_compile_ms: u128,
    pub asset_decode_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_initialization_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_adapter_request_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_device_request_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_pipeline_creation_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture_upload_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_frame_command_encode_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_submission_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_readback_wait_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_repack_ms: Option<u128>,
    pub track_evaluation_ms: u128,
    /// Aggregate backend frame-render work. Concurrent CPU workers can make
    /// this exceed wall-clock render time.
    pub frame_render_ms: u128,
    pub encoder_write_ms: u128,
    pub encoder_finalize_ms: u128,
    pub output_publish_ms: u128,
    /// Compatibility alias for `operation_total_ms`.
    pub total_ms: u128,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderSummary {
    pub output_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub frame_count: u64,
    pub audio_present: bool,
    pub preview: bool,
    pub elapsed_ms: u128,
    pub timings: RenderTimings,
    pub performance: PreparationStats,
    pub requested_render_backend: RenderBackendPreference,
    pub render_backend: RenderBackendKind,
    pub backend_fallback: Option<BackendFallback>,
    pub adapter: Option<AdapterMetadata>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderEvent {
    pub event_schema_version: u8,
    #[serde(rename = "type")]
    pub kind: String,
    pub frame: u64,
    pub total_frames: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<Diagnostic>>,
}

/// Synchronous control returned by an advanced render observer.
///
/// `Cancel` stops a pre-publication operation, aborts the encoder, removes the
/// temporary output, and returns the usual structured cancellation error. The
/// result of the post-publication `completed` event is ignored because output
/// publication has already succeeded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderObserverControl {
    #[default]
    Continue,
    Cancel,
}

#[derive(Debug)]
pub struct RenderError {
    pub diagnostic: Diagnostic,
    /// Operation warnings discovered after renderer preparation began.
    pub warnings: Vec<Diagnostic>,
    pub temporary_removed: bool,
    pub context: RenderFailureContext,
    /// Timings measured before this render-stage failure.
    pub timings: RenderTimings,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderFailureContext {
    pub stage: RenderFailureStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_completed_frame_index: Option<u64>,
    pub completed_frames: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempted_frame: Option<u64>,
    pub total_frames: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline_position: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temporary_output_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderFailureStage {
    OutputPreparation,
    AssetPreparation,
    EncoderStartup,
    FrameComposition,
    FrameWrite,
    EncoderFinalization,
    OutputPublication,
    Cancellation,
}

impl RenderFailureStage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OutputPreparation => "output_preparation",
            Self::AssetPreparation => "asset_preparation",
            Self::EncoderStartup => "encoder_startup",
            Self::FrameComposition => "frame_composition",
            Self::FrameWrite => "frame_write",
            Self::EncoderFinalization => "encoder_finalization",
            Self::OutputPublication => "output_publication",
            Self::Cancellation => "cancellation",
        }
    }
}
