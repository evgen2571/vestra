import os
from pathlib import Path
import subprocess
import sys

import vestra


def test_modern_public_imports_preserve_the_v2_api_hierarchy() -> None:
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


def test_legacy_package_is_not_importable_from_the_source_tree() -> None:
    package_root = Path(__file__).parents[1] / "python"
    environment = os.environ | {
        "PYTHONPATH": os.pathsep.join(
            path for path in (str(package_root), os.environ.get("PYTHONPATH", "")) if path
        )
    }
    subprocess.run(
        [
            sys.executable,
            "-S",
            "-c",
            "import importlib.util; assert importlib.util.find_spec('video_editor') is None",
        ],
        check=True,
        env=environment,
        capture_output=True,
        text=True,
    )


def test_public_import_and_version() -> None:
    assert vestra.__version__ == vestra.native_version()
    assert "_native" not in vestra.__all__
