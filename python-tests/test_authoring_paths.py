import os
from pathlib import Path

import pytest

from vestra import FrameRate
from vestra.authoring import ProjectBuilder


class StringPath:
    def __fspath__(self) -> str:
        return "custom.mp4"


class BytesPath:
    def __fspath__(self) -> bytes:
        return b"custom.mp4"


def builder(**changes: object) -> ProjectBuilder:
    values: dict[str, object] = {
        "width": 160, "height": 90, "frame_rate": FrameRate(30, 1),
        "output_path": "out.mp4", "duration": 1.0,
    }
    values.update(changes)
    return ProjectBuilder(**values)  # type: ignore[arg-type]


def test_paths_accept_strings_and_string_pathlikes(tmp_path: Path) -> None:
    authored = builder(output_path=StringPath(), base_directory=tmp_path)
    assert authored.to_dict()["output"]["path"] == "custom.mp4"  # type: ignore[index]
    assert authored.build().base_directory == tmp_path
    assert "base_directory" not in authored.to_dict()
    assert builder(output_path=Path("pathlib.mp4")).output_path == "pathlib.mp4"


@pytest.mark.parametrize("value", [b"out.mp4", BytesPath()])
def test_paths_reject_bytes(value: str | os.PathLike[str]) -> None:
    with pytest.raises(TypeError, match="str or PathLike"):
        builder(output_path=value)
