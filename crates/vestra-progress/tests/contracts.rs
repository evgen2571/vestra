use vestra_core::OperationId;
use vestra_progress::{ProgressMode, ProgressSink, RenderEvent, RenderStage};

#[test]
fn render_events_serialize_a_typed_lifecycle_contract() {
    let operation_id = OperationId::new();
    let event = RenderEvent::progress(operation_id, 3, 10);

    assert_eq!(event.operation_id(), operation_id);
    assert_eq!(event.stage(), Some(RenderStage::Rendering));
    assert_eq!(
        serde_json::to_value(event).expect("serializes"),
        serde_json::json!({
            "event_schema_version": 2,
            "type": "progress",
            "operation_id": operation_id.value(),
            "frame": 3,
            "total_frames": 10,
            "fraction": 0.3
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
