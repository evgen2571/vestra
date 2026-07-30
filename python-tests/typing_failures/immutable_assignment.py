from video_editor import Diagnostic, Editor, PreflightOptions, Project

project = Project.from_dict({
    "schema_version": 1,
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
