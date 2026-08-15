from vestra import AdapterInfo, BackendFallback, CancellationToken, Diagnostic, Editor, FrameRate, PreflightOptions, PrepareOptions, ProjectSnapshot, RenderEvent, RenderResult

project = ProjectSnapshot.from_dict({
    "schema_version": 3,
    "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": False, "duration_mode": "automatic"},
    "assets": [],
    "visual": {"clips": []},
})
report = Editor().validate(project)
options = PreflightOptions.for_validation()
diagnostic: Diagnostic

diagnostic.code = "changed"  # E: Property "code" defined in "Diagnostic" is read-only
report.is_valid = False  # E: Property "is_valid" defined in "ValidationReport" is read-only
options.overwrite = False  # E: Property "overwrite" defined in "PreflightOptions" is read-only
prepared = Editor().prepare(project, PrepareOptions())
frame = prepared.render_frame_number(0)
prepared.preparation_report = prepared.preparation_report  # E: Property "preparation_report" defined in "PreparedProject" is read-only
frame.width = 1  # E: Property "width" defined in "Frame" is read-only
frame.timestamp_ns = 1  # E: Property "timestamp_ns" defined in "Frame" is read-only
prepared.preparation_report.selected_backend = prepared.preparation_report.selected_backend  # E: Property "selected_backend" defined in "PreparationReport" is read-only
PrepareOptions().backend = PrepareOptions().backend  # E: Property "backend" defined in "PrepareOptions" is read-only
FrameRate(24).numerator = 1  # E: Property "numerator" defined in "FrameRate" is read-only
adapter: AdapterInfo
fallback: BackendFallback
adapter.device_type = adapter.device_type  # E: Property "device_type" defined in "AdapterInfo" is read-only
fallback.message = "changed"  # E: Property "message" defined in "BackendFallback" is read-only
result: RenderResult
result.output_path = "changed"  # E: Property "output_path" defined in "RenderResult" is read-only
token = CancellationToken()
token.is_cancelled = False  # E: Property "is_cancelled" defined in "CancellationToken" is read-only
event: RenderEvent
event.progress = 1.0  # E: Property "progress" defined in "RenderEvent" is read-only
from vestra import PreparedVideoRenderRequest
PreparedVideoRenderRequest("out.mp4").overwrite = True  # E: Property "overwrite" defined in "PreparedVideoRenderRequest" is read-only
