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

#[derive(Clone, Debug, Serialize)]
pub struct BackendFallback {
    pub code: String,
    pub stage: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RenderTimings {
    /// JSON parsing spent while constructing `Project`; zero for `from_value`.
    pub project_parse_ms: u128,
    /// Time inside `Editor::render`, excluding earlier project construction.
    pub operation_total_ms: u128,
    pub semantic_validation_ms: u128,
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

#[derive(Debug)]
pub struct RenderError {
    pub diagnostic: Diagnostic,
    pub temporary_removed: bool,
    pub context: RenderFailureContext,
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

#[derive(Clone, Copy, Debug, Serialize)]
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
