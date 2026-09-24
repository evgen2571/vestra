import vestra


def test_public_imports_expose_the_authoring_and_native_project_types() -> None:
    from vestra import Project, ProjectSnapshot
    from vestra.authoring import ProjectBuilder

    assert Project is vestra.Project
    assert ProjectSnapshot is vestra.ProjectSnapshot
    assert ProjectBuilder is vestra.authoring.ProjectBuilder


def test_project_snapshot_is_the_native_project_alias() -> None:
    import vestra._native as native
    from vestra.authoring import ProjectBuilder

    assert vestra.ProjectSnapshot is native.Project
    snapshot = ProjectBuilder(
        width=2,
        height=2,
        frame_rate=vestra.FrameRate(1, 1),
        output_path="out.mp4",
    ).build()
    assert isinstance(snapshot, vestra.ProjectSnapshot)


def test_public_import_and_version() -> None:
    assert vestra.__version__ == vestra.native_version()
    assert "_native" not in vestra.__all__
