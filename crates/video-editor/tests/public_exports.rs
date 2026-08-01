//! Compile-time contract for the supported external SDK surface.

use video_editor::{
    AdapterDeviceType, AdapterInfo, BackendFallback, BackendKind, BackendPreference,
    CancellationToken, Category, Diagnostic, Editor, EditorBuilder, EditorError, EditorErrorKind,
    Frame, FrameRate, FrameRateError, GraphicsBackend, InspectAssets, InspectAudio,
    InspectAudioClip, InspectAudioGainKeyframe, InspectAudioTrack, InspectOutput, InspectionReport,
    LoadError, PixelFormat, PreflightOptions, PreflightReport, PreparationReport,
    PreparationTimings, PrepareOptions, PreparedProject, PreparedVideoRenderRequest, Project,
    RenderEvent, RenderFailureContext, RenderFailureStage, RenderObserverControl,
    RenderPerformance, RenderRequest, RenderResult, RenderTimingScope, RenderTimings, Severity,
    ValidateResult, ValidationReport, VersionResult,
};

#[test]
fn supported_sdk_exports_are_importable_by_an_external_crate() {
    fn assert_sized<T>() {
        let _ = std::mem::size_of::<T>();
    }

    assert_sized::<Editor>();
    assert_sized::<EditorBuilder>();
    assert_sized::<EditorError>();
    assert_sized::<EditorErrorKind>();
    assert_sized::<Project>();
    assert_sized::<PreparedProject>();
    assert_sized::<PrepareOptions>();
    assert_sized::<PreparedVideoRenderRequest>();
    assert_sized::<RenderRequest>();
    assert_sized::<Frame>();
    assert_sized::<PixelFormat>();
    assert_sized::<FrameRate>();
    assert_sized::<FrameRateError>();
    assert_sized::<PreparationReport>();
    assert_sized::<PreparationTimings>();
    assert_sized::<RenderResult>();
    assert_sized::<RenderTimingScope>();
    assert_sized::<RenderTimings>();
    assert_sized::<RenderFailureContext>();
    assert_sized::<RenderFailureStage>();
    assert_sized::<RenderObserverControl>();
    assert_sized::<ValidationReport>();
    assert_sized::<PreflightReport>();
    assert_sized::<InspectionReport>();
    assert_sized::<InspectOutput>();
    assert_sized::<InspectAssets>();
    assert_sized::<InspectAudio>();
    assert_sized::<InspectAudioTrack>();
    assert_sized::<InspectAudioClip>();
    assert_sized::<InspectAudioGainKeyframe>();
    assert_sized::<ValidateResult>();
    assert_sized::<VersionResult>();
    assert_sized::<Diagnostic>();
    assert_sized::<CancellationToken>();
    assert_sized::<RenderEvent>();
    assert_sized::<BackendPreference>();
    assert_sized::<BackendKind>();
    assert_sized::<BackendFallback>();
    assert_sized::<AdapterInfo>();
    assert_sized::<AdapterDeviceType>();
    assert_sized::<RenderPerformance>();
    assert_sized::<GraphicsBackend>();
    assert_sized::<Category>();
    assert_sized::<Severity>();
    assert_sized::<LoadError>();
    assert_sized::<PreflightOptions>();
}

#[test]
fn inspection_nested_dtos_are_nameable_by_external_callers() {
    fn output(_: &InspectOutput) {}
    fn assets(_: &InspectAssets) {}
    fn audio(_: &InspectAudio) {}
    fn audio_track(_: &InspectAudioTrack) {}
    fn audio_clip(_: &InspectAudioClip) {}
    fn audio_keyframe(_: &InspectAudioGainKeyframe) {}
    fn fields(report: &InspectionReport) {
        output(&report.output);
        assets(&report.assets);
        if let Some(audio_report) = report.audio.as_ref() {
            audio(audio_report);
            for track in &audio_report.tracks {
                audio_track(track);
                for clip in &track.clips {
                    audio_clip(clip);
                    for keyframe in &clip.gain_automation {
                        audio_keyframe(keyframe);
                    }
                }
            }
        }
    }
    let _ = fields;
}
