import video_editor


def test_public_import_and_version() -> None:
    assert video_editor.__version__ == video_editor.native_version()
    assert "_native" not in video_editor.__all__
