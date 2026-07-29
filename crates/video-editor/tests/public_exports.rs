//! Compile-time contract for the supported external SDK surface.

use video_editor::{
    AdapterDeviceType, AdapterInfo, BackendFallback, BackendKind, BackendPreference,
    CancellationToken, Diagnostic, Editor, EditorBuilder, Frame, FrameRate, InspectionReport,
    PixelFormat, PreflightReport, PreparationReport, PreparationTimings, PrepareOptions,
    PreparedProject, PreparedVideoRenderRequest, Project, RenderEvent, RenderPerformance,
    RenderRequest, RenderResult, RenderTimingScope, ValidationReport,
};

#[test]
fn supported_sdk_exports_are_importable_by_an_external_crate() {
    fn assert_sized<T>() {
        let _ = std::mem::size_of::<T>();
    }

    assert_sized::<Editor>();
    assert_sized::<EditorBuilder>();
    assert_sized::<Project>();
    assert_sized::<PreparedProject>();
    assert_sized::<PrepareOptions>();
    assert_sized::<PreparedVideoRenderRequest>();
    assert_sized::<RenderRequest>();
    assert_sized::<Frame>();
    assert_sized::<PixelFormat>();
    assert_sized::<FrameRate>();
    assert_sized::<PreparationReport>();
    assert_sized::<PreparationTimings>();
    assert_sized::<RenderResult>();
    assert_sized::<RenderTimingScope>();
    assert_sized::<ValidationReport>();
    assert_sized::<PreflightReport>();
    assert_sized::<InspectionReport>();
    assert_sized::<Diagnostic>();
    assert_sized::<CancellationToken>();
    assert_sized::<RenderEvent>();
    assert_sized::<BackendPreference>();
    assert_sized::<BackendKind>();
    assert_sized::<BackendFallback>();
    assert_sized::<AdapterInfo>();
    assert_sized::<AdapterDeviceType>();
    assert_sized::<RenderPerformance>();
}
