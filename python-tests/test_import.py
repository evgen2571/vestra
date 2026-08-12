import vestra


def test_legacy_import_forwards_to_canonical_package() -> None:
    import video_editor
    from video_editor.authoring import ProjectBuilder as LegacyProjectBuilder
    from vestra.authoring import ProjectBuilder

    assert video_editor.Editor is vestra.Editor
    assert video_editor.Project is vestra.Project
    assert LegacyProjectBuilder is ProjectBuilder


def test_public_import_and_version() -> None:
    assert vestra.__version__ == vestra.native_version()
    assert "_native" not in vestra.__all__
