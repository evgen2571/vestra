#![allow(
    clippy::result_large_err,
    reason = "application errors preserve command diagnostics"
)]

use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use crate::{
    Diagnostic,
    plan::{CompileOptions, compile},
    project::ValidatedProject,
    render::{
        PreparedState, RenderBackendPreference, RenderError, RenderEvent, RenderObserverControl,
        RenderOptions, RenderSummary, prepare_for_video, render_prepared, render_prepared_frame,
    },
};

#[derive(Clone, Debug)]
pub struct RenderRequest {
    pub output_override: Option<PathBuf>,
    pub overwrite: bool,
    pub preview: bool,
    pub cancelled: Arc<AtomicBool>,
    pub backend_preference: RenderBackendPreference,
}

#[derive(Debug)]
pub enum ApplicationRenderError {
    Plan {
        diagnostic: Diagnostic,
        plan_compile_elapsed_ms: u128,
    },
    Render {
        error: Box<RenderError>,
        plan_compile_elapsed_ms: u128,
    },
}

/// Private bridge between SDK preparation and a one-shot video operation.
/// It owns every reusable visual resource and contains no output or encoder.
pub(crate) struct PreparedRender {
    metadata: PreparedRenderMetadata,
    prepared: PreparedState,
    /// Immutable context produced before any encoder or output operation.
    preparation_warnings: Vec<Diagnostic>,
    requested_backend: RenderBackendPreference,
    selected_backend: crate::render::RenderBackendKind,
    backend_fallback: Option<crate::render::BackendFallback>,
    preparation_timings: PreparationTimings,
    plan_compile_elapsed_ms: u128,
}

/// Result fields retained from validation. Keeping this narrow avoids cloning
/// the resolved project (asset maps, media metadata, and diagnostics) for each
/// video operation.
#[derive(Clone, Debug)]
pub(crate) struct PreparedRenderMetadata {
    pub(crate) frame_rate: String,
    pub(crate) visual_clip_count: usize,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frame_rate_ratio: (u64, u64),
    pub(crate) frame_count: u64,
    pub(crate) duration: Duration,
    pub(crate) decoded_asset_count: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PreparationTimings {
    pub(crate) validation_ms: u128,
    pub(crate) preflight_ms: u128,
    pub(crate) plan_compile_ms: u128,
    pub(crate) renderer: crate::render::PreparationTimings,
}

impl PreparedRender {
    pub(crate) const fn requested_backend(&self) -> RenderBackendPreference {
        self.requested_backend
    }
    pub(crate) const fn selected_backend(&self) -> crate::render::RenderBackendKind {
        self.selected_backend
    }
    pub(crate) fn prepared_backend_fallback(&self) -> Option<crate::render::BackendFallback> {
        self.backend_fallback.clone()
    }
    pub(crate) fn preparation_warnings(&self) -> &[Diagnostic] {
        &self.preparation_warnings
    }
    pub(crate) fn adapter_metadata(&self) -> Option<crate::render::AdapterMetadata> {
        self.prepared.adapter_metadata()
    }

    pub(crate) const fn preparation_timings(&self) -> PreparationTimings {
        self.preparation_timings
    }

    pub(crate) fn result_metadata(&self) -> PreparedRenderMetadata {
        self.metadata.clone()
    }

    pub(crate) fn render_frame(
        &mut self,
        frame_number: u64,
    ) -> Result<crate::render::CompletedFrame, ApplicationRenderError> {
        render_prepared_frame(&mut self.prepared, frame_number).map_err(|error| {
            ApplicationRenderError::Render {
                error: Box::new(error),
                plan_compile_elapsed_ms: self.plan_compile_elapsed_ms,
            }
        })
    }
}

pub(crate) fn prepare_project(
    validated: ValidatedProject,
    request: &RenderRequest,
    preparation_warnings: Vec<Diagnostic>,
    preparation_timings: PreparationTimings,
) -> Result<PreparedRender, ApplicationRenderError> {
    let compilation_started = Instant::now();
    let plan = compile(
        &validated,
        CompileOptions {
            preview: request.preview,
        },
    )
    .map_err(|diagnostic| ApplicationRenderError::Plan {
        diagnostic,
        plan_compile_elapsed_ms: compilation_started.elapsed().as_millis(),
    })?;
    let plan_compile_elapsed_ms = compilation_started.elapsed().as_millis();
    let prepared = prepare_for_video(plan, request.backend_preference).map_err(|error| {
        ApplicationRenderError::Render {
            error: Box::new(error),
            plan_compile_elapsed_ms,
        }
    })?;
    let metadata = PreparedRenderMetadata {
        frame_rate: validated.project.output.frame_rate.display(),
        visual_clip_count: validated.visual_counts().0,
        width: 0,
        height: 0,
        frame_rate_ratio: validated.frame_rate,
        frame_count: validated.frame_count,
        duration: Duration::from_nanos(u64::try_from(validated.duration_nanos).map_err(|_| {
            ApplicationRenderError::Plan {
                diagnostic: Diagnostic::error(
                    "MVP-TIMELINE-OVERFLOW",
                    crate::Category::Render,
                    "project duration cannot be represented",
                    "",
                ),
                plan_compile_elapsed_ms,
            }
        })?),
        decoded_asset_count: 0,
    };
    let (width, height, frame_rate_ratio, frame_count, _duration, decoded_asset_count) =
        prepared.frame_details();
    let metadata = PreparedRenderMetadata {
        width,
        height,
        frame_rate_ratio,
        frame_count,
        decoded_asset_count,
        ..metadata
    };
    let renderer_preparation_timings = prepared.preparation_timings();
    Ok(PreparedRender {
        metadata,
        requested_backend: prepared.requested_backend(),
        selected_backend: prepared.selected_backend(),
        backend_fallback: prepared.backend_fallback().cloned(),
        prepared,
        preparation_warnings,
        preparation_timings: PreparationTimings {
            plan_compile_ms: plan_compile_elapsed_ms,
            renderer: renderer_preparation_timings,
            ..preparation_timings
        },
        plan_compile_elapsed_ms,
    })
}

pub(crate) fn render_prepared_project(
    prepared: &mut PreparedRender,
    request: RenderRequest,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
) -> Result<RenderSummary, ApplicationRenderError> {
    let summary = render_prepared(
        &mut prepared.prepared,
        &RenderOptions {
            output_override: request.output_override,
            overwrite: request.overwrite,
            cancelled: request.cancelled,
            #[cfg(test)]
            backend_preference: request.backend_preference,
        },
        emit,
    )
    .map_err(|error| ApplicationRenderError::Render {
        error: Box::new(error),
        plan_compile_elapsed_ms: prepared.plan_compile_elapsed_ms,
    })?;
    debug_assert_eq!(summary.requested_render_backend, prepared.requested_backend);
    debug_assert_eq!(summary.render_backend, prepared.selected_backend);
    debug_assert_eq!(summary.backend_fallback, prepared.backend_fallback);
    Ok(summary)
}

#[cfg(test)]
pub(crate) fn render_prepared_project_with_sink<S, SF>(
    prepared: &mut PreparedRender,
    request: RenderRequest,
    emit: &mut dyn FnMut(RenderEvent) -> RenderObserverControl,
    start_sink: SF,
) -> Result<RenderSummary, ApplicationRenderError>
where
    S: video_editor_media::FrameSink,
    SF: FnOnce(
        &video_editor_media::EncoderSettings,
        &std::path::Path,
    ) -> Result<S, video_editor_media::MediaError>,
{
    let summary = crate::render::render_prepared_with_sink(
        &mut prepared.prepared,
        &RenderOptions {
            output_override: request.output_override,
            overwrite: request.overwrite,
            cancelled: request.cancelled,
            backend_preference: request.backend_preference,
        },
        emit,
        start_sink,
    )
    .map_err(|error| ApplicationRenderError::Render {
        error: Box::new(error),
        plan_compile_elapsed_ms: prepared.plan_compile_elapsed_ms,
    })?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use video_editor_media::{FrameSink, MediaError, SinkResult};

    use super::*;
    use crate::{
        project::{ValidationOptions, load_and_validate},
        render::CompletedFrame,
    };

    type CapturedFrames = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

    struct CountingSink {
        frames: u64,
        temporary_path: std::path::PathBuf,
    }

    struct CaptureSink {
        frames: CapturedFrames,
        temporary_path: std::path::PathBuf,
    }

    impl FrameSink for CaptureSink {
        fn write_frame(&mut self, frame: &CompletedFrame) -> Result<(), MediaError> {
            self.frames
                .lock()
                .expect("capture lock")
                .push((frame.frame_number, frame.rgba.clone()));
            Ok(())
        }

        fn finish(&mut self) -> Result<SinkResult, MediaError> {
            std::fs::write(&self.temporary_path, b"captured frames")
                .map_err(MediaError::Publication)?;
            Ok(SinkResult {
                frames_written: self.frames.lock().expect("capture lock").len() as u64,
            })
        }

        fn abort(&mut self) -> Result<(), MediaError> {
            Ok(())
        }
    }

    impl FrameSink for CountingSink {
        fn write_frame(&mut self, _frame: &CompletedFrame) -> Result<(), MediaError> {
            self.frames += 1;
            Ok(())
        }

        fn finish(&mut self) -> Result<SinkResult, MediaError> {
            std::fs::write(&self.temporary_path, b"prepared operation")
                .map_err(MediaError::Publication)?;
            Ok(SinkResult {
                frames_written: self.frames,
            })
        }

        fn abort(&mut self) -> Result<(), MediaError> {
            Ok(())
        }
    }

    #[test]
    fn application_bridge_prepares_once_and_renders_two_operations() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/projects/animation-effects.json");
        let validated =
            load_and_validate(&fixture, &ValidationOptions::default()).expect("fixture validates");
        let request = RenderRequest {
            backend_preference: RenderBackendPreference::Cpu,
            cancelled: Arc::new(AtomicBool::new(false)),
            output_override: None,
            overwrite: true,
            preview: false,
        };
        let mut prepared = prepare_project(
            validated,
            &request,
            Vec::new(),
            PreparationTimings::default(),
        )
        .expect("one private preparation succeeds");
        let workspace = tempfile::tempdir().expect("temporary output directory");
        let sink_creations = Arc::new(AtomicUsize::new(0));
        let mut frame_counts = Vec::new();
        for name in ["first.mp4", "second.mp4"] {
            let output = workspace.path().join(name);
            let sink_creations = Arc::clone(&sink_creations);
            let summary = render_prepared_project_with_sink(
                &mut prepared,
                RenderRequest {
                    output_override: Some(output.clone()),
                    ..request.clone()
                },
                &mut |_| RenderObserverControl::Continue,
                move |_settings, temporary_path| {
                    sink_creations.fetch_add(1, Ordering::Relaxed);
                    Ok(CountingSink {
                        frames: 0,
                        temporary_path: temporary_path.to_path_buf(),
                    })
                },
            )
            .expect("prepared application operation succeeds");
            assert!(output.exists(), "operation publishes its own output");
            frame_counts.push(summary.performance.submitted_frames);
        }
        assert_eq!(sink_creations.load(Ordering::Relaxed), 2);
        assert_eq!(frame_counts.len(), 2);
        assert!(frame_counts[0] > 0);
        assert_eq!(frame_counts[0], frame_counts[1]);
    }

    #[test]
    fn prepared_operations_report_cache_deltas_and_reuse_static_layers() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/projects/audio-static-mix.json");
        let validated =
            load_and_validate(&fixture, &ValidationOptions::default()).expect("fixture validates");
        let request = RenderRequest {
            backend_preference: RenderBackendPreference::Cpu,
            cancelled: Arc::new(AtomicBool::new(false)),
            output_override: None,
            overwrite: true,
            preview: false,
        };
        let mut prepared = prepare_project(
            validated,
            &request,
            Vec::new(),
            PreparationTimings::default(),
        )
        .expect("prepare CPU");
        let workspace = tempfile::tempdir().expect("temporary output directory");
        let mut summaries = Vec::new();
        let mut operations = Vec::new();
        for name in ["first.mp4", "second.mp4"] {
            let captured = Arc::new(Mutex::new(Vec::new()));
            let sink_frames = Arc::clone(&captured);
            let summary = render_prepared_project_with_sink(
                &mut prepared,
                RenderRequest {
                    output_override: Some(workspace.path().join(name)),
                    ..request.clone()
                },
                &mut |_| RenderObserverControl::Continue,
                move |_settings, temporary_path| {
                    Ok(CaptureSink {
                        frames: sink_frames,
                        temporary_path: temporary_path.to_path_buf(),
                    })
                },
            )
            .expect("prepared operation succeeds");
            summaries.push(summary);
            operations.push(
                Arc::try_unwrap(captured)
                    .expect("only sink retains capture")
                    .into_inner()
                    .expect("capture lock"),
            );
        }

        let first = &summaries[0].performance;
        let second = &summaries[1].performance;
        // `canvas` is static for two seconds at 30 fps: one population, then
        // 59 reuse hits in the first operation and 60 in the second.
        assert_eq!(
            (first.static_cache_misses, first.static_cache_hits),
            (1, 59)
        );
        assert_eq!(first.static_cache_population_renders, 1);
        assert_eq!(first.static_layers_rendered, 1);
        assert_eq!(
            (first.static_cache_entries, first.static_cached_bytes),
            (1, 64 * 64 * 4),
        );
        assert_eq!(
            (second.static_cache_misses, second.static_cache_hits),
            (0, 60)
        );
        assert_eq!(second.static_cache_population_renders, 0);
        assert_eq!(second.static_layers_rendered, 0);
        assert_eq!(
            (second.static_cache_entries, second.static_cached_bytes),
            (1, 64 * 64 * 4),
        );
        assert_eq!(operations[0], operations[1]);
        let lifetime = prepared.prepared.backend_stats();
        assert_eq!(
            lifetime.static_cache_misses,
            first.static_cache_misses + second.static_cache_misses
        );
        assert_eq!(
            lifetime.static_cache_hits,
            first.static_cache_hits + second.static_cache_hits
        );
        assert_eq!(
            lifetime.static_cache_population_renders,
            first.static_cache_population_renders + second.static_cache_population_renders,
        );
        assert_eq!(
            lifetime.static_layers_rendered,
            first.static_layers_rendered + second.static_layers_rendered,
        );

        for frame_number in [50, 5, 30] {
            let random = prepared.render_frame(frame_number).expect("random frame");
            assert_eq!(random.rgba, operations[0][frame_number as usize].1);
        }
    }

    #[test]
    fn random_cpu_frame_matches_the_pre_encoding_video_frame() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/wgpu-small-rgba.json");
        let validated =
            load_and_validate(&fixture, &ValidationOptions::default()).expect("fixture validates");
        let request = RenderRequest {
            backend_preference: RenderBackendPreference::Cpu,
            cancelled: Arc::new(AtomicBool::new(false)),
            output_override: None,
            overwrite: true,
            preview: false,
        };
        let mut prepared = prepare_project(
            validated,
            &request,
            Vec::new(),
            PreparationTimings::default(),
        )
        .expect("prepare CPU");
        let random = prepared.render_frame(0).expect("random frame");
        let workspace = tempfile::tempdir().expect("temporary output directory");
        let output = workspace.path().join("parity.mp4");
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink_frames = Arc::clone(&captured);
        render_prepared_project_with_sink(
            &mut prepared,
            RenderRequest {
                output_override: Some(output),
                ..request
            },
            &mut |_| RenderObserverControl::Continue,
            move |_settings, temporary_path| {
                Ok(CaptureSink {
                    frames: sink_frames,
                    temporary_path: temporary_path.to_path_buf(),
                })
            },
        )
        .expect("video path");
        let video = captured.lock().expect("capture lock");
        assert_eq!(video.len(), 1);
        assert_eq!(video[0].0, random.frame_number);
        assert_eq!(video[0].1, random.rgba);
    }

    #[test]
    fn random_cpu_frames_match_every_pre_encoding_animation_effects_frame() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/projects/animation-effects.json");
        let validated =
            load_and_validate(&fixture, &ValidationOptions::default()).expect("fixture validates");
        let request = RenderRequest {
            backend_preference: RenderBackendPreference::Cpu,
            cancelled: Arc::new(AtomicBool::new(false)),
            output_override: None,
            overwrite: true,
            preview: false,
        };
        let mut prepared = prepare_project(
            validated,
            &request,
            Vec::new(),
            PreparationTimings::default(),
        )
        .expect("prepare CPU");
        let workspace = tempfile::tempdir().expect("temporary output directory");
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink_frames = Arc::clone(&captured);
        render_prepared_project_with_sink(
            &mut prepared,
            RenderRequest {
                output_override: Some(workspace.path().join("parity.mp4")),
                ..request
            },
            &mut |_| RenderObserverControl::Continue,
            move |_settings, temporary_path| {
                Ok(CaptureSink {
                    frames: sink_frames,
                    temporary_path: temporary_path.to_path_buf(),
                })
            },
        )
        .expect("video path");
        let video = captured.lock().expect("capture lock");
        assert!(!video.is_empty());
        for (frame_number, pixels) in video.iter() {
            let random = prepared.render_frame(*frame_number).expect("random frame");
            assert_eq!(&random.rgba, pixels, "frame {frame_number}");
        }
    }
}
