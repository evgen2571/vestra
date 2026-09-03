import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

def _run_script(script: str, *args: str) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    package_root = Path(__file__).parents[1] / "python"
    environment["PYTHONPATH"] = os.pathsep.join(
        path for path in (str(package_root), environment.get("PYTHONPATH", "")) if path
    )
    return subprocess.run(
        [sys.executable, "-c", script, *args],
        check=False,
        capture_output=True,
        text=True,
        env=environment,
    )


def test_import_does_not_install_a_subscriber() -> None:
    result = _run_script(
        "import vestra; vestra.configure_logging(level='info'); print('configured')"
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "configured"


def test_python_logging_configuration_supports_human_file_output(tmp_path: Path) -> None:
    log_path = tmp_path / "vestra.log"
    result = _run_script(
        """
import sys
from pathlib import Path
import vestra
path = Path(sys.argv[1])
vestra.configure_logging(level="info", file=path)
vestra.Project(size=(2, 2), fps=1, duration=1, base_directory=path.parent).render(
    path.with_suffix(".mp4"), backend="cpu", overwrite=True
)
""",
        str(log_path),
    )
    assert result.returncode == 0, result.stderr
    assert "render started" in log_path.read_text()


def test_python_logging_configuration_supports_json_file_output(tmp_path: Path) -> None:
    log_path = tmp_path / "vestra.jsonl"
    result = _run_script(
        """
import sys
from pathlib import Path
import vestra
path = Path(sys.argv[1])
vestra.configure_logging(level="info", format="json", file=path)
vestra.Project(size=(2, 2), fps=1, duration=1, base_directory=path.parent).render(
    path.with_suffix(".mp4"), backend="cpu", overwrite=True
)
""",
        str(log_path),
    )
    assert result.returncode == 0, result.stderr
    assert log_path.is_file()
    records = [json.loads(line) for line in log_path.read_text().splitlines()]
    assert any(record["target"] == "vestra.render" for record in records)


@pytest.mark.parametrize(
    ("argument", "value"),
    [("level", "loud"), ("format", "xml"), ("output", "socket")],
)
def test_python_logging_configuration_rejects_invalid_values(
    argument: str, value: str
) -> None:
    result = _run_script(
        f"import vestra; vestra.configure_logging({argument}={value!r})"
    )
    assert result.returncode != 0
    assert "must be" in result.stderr


def test_python_logging_configuration_validates_file_path(tmp_path: Path) -> None:
    result = _run_script(
        "import vestra; vestra.configure_logging(file=__import__('sys').argv[1])",
        str(tmp_path / "missing" / "vestra.log"),
    )
    assert result.returncode != 0
    assert "cannot open log file" in result.stderr


def test_python_logging_configuration_rejects_repeated_initialization() -> None:
    result = _run_script(
        "import vestra; vestra.configure_logging(); vestra.configure_logging()"
    )
    assert result.returncode != 0
    assert "global tracing subscriber" in result.stderr
