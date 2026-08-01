import video_editor

PROJECT = {
    "schema_version": 2,
    "output": {"path": "out.mp4", "width": 2, "height": 2, "frame_rate": "30/1", "background": "#000000", "quality": "balanced", "audio": False, "duration_mode": "automatic"},
    "assets": [], "visual": {"clips": []},
}


def test_validation_is_a_report() -> None:
    report = video_editor.Editor().validate(video_editor.Project.from_dict(PROJECT))
    assert report.is_valid
    assert report.errors == ()
