import os
from pathlib import Path
import subprocess
import sys

import vestra


def test_legacy_import_forwards_to_canonical_package() -> None:
    import video_editor
    from video_editor.authoring import ProjectBuilder as LegacyProjectBuilder
    from vestra.authoring import ProjectBuilder

    assert video_editor.Editor is vestra.Editor
    assert video_editor.Project is vestra.ProjectSnapshot
    assert LegacyProjectBuilder is ProjectBuilder
    assert video_editor.__all__ == vestra.__all__
    assert video_editor.__all__ is not vestra.__all__
    assert all(
        getattr(video_editor, name) is getattr(vestra, name)
        for name in vestra.__all__
        if name != "Project"
    )


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


def test_compatibility_import_orders_work_in_clean_processes() -> None:
    package_root = Path(__file__).parents[1] / "python"
    environment = os.environ | {
        "PYTHONPATH": os.pathsep.join(
            path for path in (str(package_root), os.environ.get("PYTHONPATH", "")) if path
        )
    }
    snippets = (
        "import vestra; import vestra.authoring; import video_editor.authoring; "
        "assert video_editor.Project is vestra.ProjectSnapshot",
        "import video_editor.authoring; import vestra; import vestra.authoring; "
        "assert video_editor.Project is vestra.ProjectSnapshot",
    )
    for snippet in snippets:
        subprocess.run(
            [sys.executable, "-c", snippet],
            check=True,
            env=environment,
            capture_output=True,
            text=True,
        )


def test_public_import_and_version() -> None:
    assert vestra.__version__ == vestra.native_version()
    assert "_native" not in vestra.__all__
