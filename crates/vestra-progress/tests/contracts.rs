use vestra_core::OperationId;
use vestra_progress::{
    ProgressMode, ProgressSink, RenderEvent, RenderStage, TerminalEnvironment, TerminalProgress,
};

#[test]
fn render_events_serialize_a_typed_lifecycle_contract() {
    let operation_id = OperationId::new();
    let event = RenderEvent::progress(operation_id, 3, 10);

    assert_eq!(event.operation_id(), operation_id);
    assert_eq!(event.stage(), Some(RenderStage::Rendering));
    assert_eq!(
        serde_json::to_value(event).expect("serializes"),
        serde_json::json!({
            "event_schema_version": 1,
            "type": "progress",
            "operation_id": operation_id.value(),
            "frame": 3,
            "total_frames": 10,
            "fraction": 0.3
        })
    );
}

#[test]
fn started_event_serializes_unknown_total_as_null() {
    let operation_id = OperationId::new();
    let event = RenderEvent::started(operation_id, None, "output.mp4".into());

    assert_eq!(
        serde_json::to_value(event).expect("serializes"),
        serde_json::json!({
            "event_schema_version": 1,
            "type": "started",
            "operation_id": operation_id.value(),
            "total_frames": null,
            "output_path": "output.mp4"
        })
    );
}

#[test]
fn stages_have_a_stable_order_and_progress_modes_are_presentation_policy() {
    assert!(RenderStage::Preparing < RenderStage::Rendering);
    assert!(RenderStage::Rendering < RenderStage::Encoding);
    assert!(RenderStage::Encoding < RenderStage::Finalizing);
    assert_eq!(ProgressMode::default(), ProgressMode::Auto);
}

#[test]
fn progress_constructor_keeps_frame_and_fraction_within_the_rendering_range() {
    let event = RenderEvent::progress(OperationId::new(), 11, 10);
    let RenderEvent::Progress {
        frame,
        total_frames,
        fraction,
        ..
    } = event
    else {
        panic!("expected progress event");
    };
    assert_eq!((frame, total_frames, fraction), (10, 10, 1.0));
}

#[test]
fn no_progress_sink_observes_events_without_side_effects() {
    let mut sink = vestra_progress::NoProgress;
    sink.on_event(&RenderEvent::cancelled(OperationId::new()));
}

#[test]
fn terminal_policy_only_selects_auto_for_a_supported_interactive_stderr() {
    let interactive = TerminalEnvironment::new(true, false, false, 80);
    let redirected = TerminalEnvironment::new(false, false, false, 80);
    let ci = TerminalEnvironment::new(true, true, false, 80);
    let dumb = TerminalEnvironment::new(true, false, true, 80);

    assert!(TerminalProgress::with_environment(ProgressMode::Auto, interactive).is_some());
    assert!(TerminalProgress::with_environment(ProgressMode::Auto, redirected).is_none());
    assert!(TerminalProgress::with_environment(ProgressMode::Auto, ci).is_none());
    assert!(TerminalProgress::with_environment(ProgressMode::Auto, dumb).is_none());
    assert!(TerminalProgress::with_environment(ProgressMode::Disabled, interactive).is_none());
    assert!(TerminalProgress::with_environment(ProgressMode::Terminal, redirected).is_some());
}

#[test]
fn terminal_progress_tracks_unknown_totals_and_does_not_finish_on_full_rendering() {
    let operation_id = OperationId::new();
    let output = "output.mp4".into();
    let environment = TerminalEnvironment::new(false, false, false, 80);
    let mut progress = TerminalProgress::with_environment(ProgressMode::Terminal, environment)
        .expect("explicit terminal mode creates a safe sink");

    progress.on_event(&RenderEvent::started(operation_id, None, output));
    assert_eq!(progress.total_frames(), None);
    assert_eq!(progress.stage(), None);
    progress.on_event(&RenderEvent::stage_changed(
        operation_id,
        RenderStage::Rendering,
    ));
    progress.on_event(&RenderEvent::progress(operation_id, 4, 10));
    assert_eq!(progress.stage(), Some(RenderStage::Rendering));
    assert_eq!(progress.frame(), Some(4));
    assert_eq!(progress.total_frames(), Some(10));
    progress.on_event(&RenderEvent::progress(operation_id, 10, 10));
    assert!(!progress.is_finished());
    progress.on_event(&RenderEvent::stage_changed(
        operation_id,
        RenderStage::Encoding,
    ));
    progress.on_event(&RenderEvent::stage_changed(
        operation_id,
        RenderStage::Finalizing,
    ));
    progress.on_event(&RenderEvent::completed(operation_id, "output.mp4".into()));
    assert!(progress.is_finished());
}

#[test]
fn terminal_progress_finalizes_cancelled_and_failed_operations_without_diagnostics() {
    let operation_id = OperationId::new();
    let environment = TerminalEnvironment::new(false, false, false, 80);
    let mut cancelled = TerminalProgress::with_environment(ProgressMode::Terminal, environment)
        .expect("explicit terminal mode creates a safe sink");
    cancelled.on_event(&RenderEvent::started(
        operation_id,
        Some(10),
        "output.mp4".into(),
    ));
    cancelled.on_event(&RenderEvent::cancelled(operation_id));
    assert!(cancelled.is_finished());

    let mut failed = TerminalProgress::with_environment(ProgressMode::Terminal, environment)
        .expect("explicit terminal mode creates a safe sink");
    failed.on_event(&RenderEvent::started(
        operation_id,
        Some(10),
        "output.mp4".into(),
    ));
    failed.on_event(&RenderEvent::failed(operation_id));
    assert!(failed.is_finished());
}
