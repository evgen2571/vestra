use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    Diagnostic,
    application::Inspection,
    render::{BackendFallback, RenderBackendPreference, RenderSummary, RenderTimings},
};

#[derive(Clone, Debug, Serialize)]
pub struct ValidateResult {
    pub project: PathBuf,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VersionResult {
    pub editor_version: &'static str,
}

#[must_use]
pub const fn version_result() -> VersionResult {
    VersionResult {
        editor_version: env!("CARGO_PKG_VERSION"),
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectResult {
    pub project: PathBuf,
    pub name: Option<String>,
    pub output: InspectOutput,
    pub assets: InspectAssets,
    pub visual_clips: usize,
    pub flashes: usize,
    pub transitions: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<InspectAudio>,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectOutput {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frame_rate: String,
    pub duration_mode: String,
    pub duration: f64,
    pub total_frames: u64,
    pub preview: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectAssets {
    pub images: usize,
    pub audio: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectAudio {
    pub track_count: usize,
    pub clip_count: usize,
    pub end: f64,
    pub tracks: Vec<InspectAudioTrack>,
    pub effects: Vec<InspectAudioEffect>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectAudioEffect {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectAudioTrack {
    pub id: String,
    pub mute: bool,
    pub gain: f64,
    pub clips: Vec<InspectAudioClip>,
    pub effects: Vec<InspectAudioEffect>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectAudioClip {
    pub id: String,
    pub asset: String,
    pub start: f64,
    pub end: f64,
    pub trim_start: f64,
    pub trim_end: f64,
    pub mute: bool,
    pub gain: f64,
    pub gain_automation: Vec<InspectAudioGainKeyframe>,
    pub fade_in: f64,
    pub fade_out: f64,
    pub fade_in_curve: vestra_core::project::AudioFadeCurve,
    pub fade_out_curve: vestra_core::project::AudioFadeCurve,
    pub effects: Vec<InspectAudioEffect>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectAudioGainKeyframe {
    pub time: f64,
    pub gain: f64,
    pub interpolation: vestra_core::project::AudioGainInterpolation,
}

pub fn inspect_result(path: &Path, inspection: Inspection) -> InspectResult {
    let inspect_effects = |effects: &[vestra_core::project::AudioEffect]| {
        effects
            .iter()
            .map(|effect| match effect {
                vestra_core::project::AudioEffect::ParametricEq { id, .. } => InspectAudioEffect {
                    id: id.clone(),
                    kind: "parametric_eq".to_owned(),
                },
                vestra_core::project::AudioEffect::PlaybackSpeed { id, .. } => InspectAudioEffect {
                    id: id.clone(),
                    kind: "playback_speed".to_owned(),
                },
                vestra_core::project::AudioEffect::BassBoost { id, .. } => InspectAudioEffect {
                    id: id.clone(),
                    kind: "bass_boost".to_owned(),
                },
            })
            .collect::<Vec<_>>()
    };
    let audio = inspection.validated.project.audio.as_ref().map(|timeline| {
        let tracks = timeline
            .tracks
            .iter()
            .map(|track| InspectAudioTrack {
                id: track.id.clone(),
                mute: track.mute,
                gain: track.gain,
                effects: inspect_effects(&track.effects),
                clips: track
                    .clips
                    .iter()
                    .filter_map(|clip| {
                        inspection.processed_audio_durations.get(&clip.id).map(
                            |processed_duration| {
                                let trim_end = clip.trim_end.unwrap_or_else(|| {
                                    inspection
                                        .validated
                                        .audio_durations
                                        .get(&clip.asset)
                                        .copied()
                                        .unwrap_or(clip.trim_start)
                                });
                                InspectAudioClip {
                                    id: clip.id.clone(),
                                    asset: clip.asset.clone(),
                                    start: clip.start,
                                    end: clip.start + processed_duration,
                                    trim_start: clip.trim_start,
                                    trim_end,
                                    mute: clip.mute,
                                    gain: clip.gain,
                                    gain_automation: clip.gain_automation.as_ref().map_or_else(
                                        Vec::new,
                                        |automation| {
                                            automation
                                                .keyframes
                                                .iter()
                                                .map(|keyframe| InspectAudioGainKeyframe {
                                                    time: keyframe.time,
                                                    gain: keyframe.gain,
                                                    interpolation: keyframe.interpolation,
                                                })
                                                .collect()
                                        },
                                    ),
                                    fade_in: clip.fade_in,
                                    fade_out: clip.fade_out,
                                    fade_in_curve: clip.fade_in_curve,
                                    fade_out_curve: clip.fade_out_curve,
                                    effects: inspect_effects(&clip.effects),
                                }
                            },
                        )
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        InspectAudio {
            track_count: tracks.len(),
            clip_count: tracks.iter().map(|track| track.clips.len()).sum(),
            end: inspection.audio_end.unwrap_or(0.0),
            tracks,
            effects: inspect_effects(&timeline.effects),
        }
    });
    InspectResult {
        project: path.to_path_buf(),
        name: inspection.validated.project.name.clone(),
        output: InspectOutput {
            path: inspection.output_path,
            width: inspection.width,
            height: inspection.height,
            frame_rate: inspection.validated.project.output.frame_rate.display(),
            duration_mode: match inspection.validated.project.output.duration_mode {
                vestra_core::project::DurationMode::Automatic => "automatic".to_owned(),
                vestra_core::project::DurationMode::Explicit => "explicit".to_owned(),
            },
            duration: inspection.validated.duration,
            total_frames: inspection.validated.frame_count,
            preview: inspection.preview,
        },
        assets: InspectAssets {
            images: inspection.image_count,
            audio: inspection.audio_count,
        },
        visual_clips: inspection.validated.visual_counts().0,
        flashes: inspection.validated.visual_counts().1,
        transitions: inspection.validated.visual_counts().2,
        audio,
        warnings: inspection.validated.warnings,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderResult {
    pub editor_version: &'static str,
    pub project: PathBuf,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frame_rate: String,
    pub duration: f64,
    pub total_frames: u64,
    pub visual_clip_count: usize,
    pub audio_present: bool,
    pub preview: bool,
    pub elapsed_ms: u128,
    /// Identifies whether timing fields include preparation.
    pub timing_scope: RenderTimingScope,
    pub timings: RenderTimings,
    pub performance: crate::RenderPerformance,
    pub requested_render_backend: RenderBackendPreference,
    pub render_backend: &'static str,
    pub encoder_backend: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_fallback: Option<BackendFallback>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<crate::AdapterInfo>,
    pub warnings: Vec<Diagnostic>,
}

/// Scope of the timing fields in a render result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderTimingScope {
    OneShot,
    PreparedOperation,
}

impl RenderTimingScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneShot => "one_shot",
            Self::PreparedOperation => "prepared_operation",
        }
    }
}

pub fn render_result(
    project: &Path,
    metadata: crate::application::render::PreparedRenderMetadata,
    summary: RenderSummary,
    warnings: Vec<Diagnostic>,
) -> RenderResult {
    RenderResult {
        editor_version: env!("CARGO_PKG_VERSION"),
        project: project.to_path_buf(),
        output: summary.output_path,
        width: summary.width,
        height: summary.height,
        frame_rate: metadata.frame_rate,
        duration: summary.duration,
        total_frames: summary.frame_count,
        visual_clip_count: metadata.visual_clip_count,
        audio_present: summary.audio_present,
        preview: summary.preview,
        elapsed_ms: summary.elapsed_ms,
        timing_scope: RenderTimingScope::OneShot,
        timings: summary.timings,
        performance: summary.performance.into(),
        requested_render_backend: summary.requested_render_backend,
        render_backend: summary.render_backend.as_str(),
        encoder_backend: "ffmpeg",
        backend_fallback: summary.backend_fallback,
        adapter: summary.adapter.map(Into::into),
        warnings,
    }
}
