from pathlib import Path

import pytest

import vestra
from vestra.sources import Color, Image


class UnknownSource(vestra.Source):
    def __init__(self, value: int) -> None:
        self.value = value


def test_project_builds_root_graph_and_canonical_sources(tmp_path: Path) -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1, base_directory=tmp_path)
    image = Image("frames/../frames/red.png")
    first = project.root.add(image, id="hero")
    second = project.root.add(Color("#00FF00"), z=1, name="green")

    snapshot = project.snapshot()
    data = snapshot.to_dict()
    assert isinstance(project.root, vestra.Composition)
    assert first.composition is project.root
    assert first.source is not image
    assert [clip["id"] for clip in data["visual"]["clips"]] == ["hero", "layer-000002"]
    assert data["assets"] == [
        {"id": "image-000001", "type": "image", "source": "frames/../frames/red.png"}
    ]
    assert data["visual"]["clips"][0]["source"] == {
        "type": "image",
        "asset": "image-000001",
    }
    assert data["visual"]["clips"][1]["source"] == {
        "type": "solid_color",
        "colour": "#00ff00",
    }
    assert second.name == "green"


def test_equivalent_image_paths_share_one_asset_without_probing() -> None:
    project = vestra.Project(size=(2, 2), fps=(30, 1), duration=1)
    project.root.add(Image("a/../b.png"))
    project.root.add(Image("b.png"), start=0.5)
    assert len(project.snapshot().to_dict()["assets"]) == 1


def test_reused_source_has_independent_layer_state() -> None:
    project = vestra.Project(size=(2, 2), fps=30, duration=2)
    source = Color("#ffffff")
    first = project.root.add(source)
    second = project.root.add(source, start=1, opacity=0.5)
    first.opacity = 0.25
    assert first.opacity.value == 0.25
    assert second.opacity.value == 0.5
    assert first.source is not second.source


def test_duplicate_ids_and_invalid_add_are_atomic() -> None:
    project = vestra.Project(size=(2, 2), fps=30, duration=1)
    project.root.add(Color("#ffffff"), id="same")
    before = project.snapshot().to_dict()
    with pytest.raises(ValueError, match="duplicate"):
        project.root.add(Color("#000000"), id="same")
    assert project.snapshot().to_dict() == before


def test_duration_must_be_inferred_or_explicit() -> None:
    project = vestra.Project(size=(2, 2), fps=30)
    with pytest.raises(ValueError, match="duration"):
        project.root.add(Color("#ffffff"))
    layer = project.root.add(Color("#ffffff"), duration=1)
    assert layer.duration == 1


def test_snapshot_validate_and_cpu_frame_render(tmp_path: Path) -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1, base_directory=tmp_path)
    project.root.add(Color("#ff0000"))
    report = project.validate()
    assert report.is_valid
    frame = project.render_frame(0, backend="cpu")
    assert frame.width == 2 and frame.height == 2
    assert frame.to_bytes()[:4] == bytes((255, 0, 0, 255))


def test_render_requires_explicit_output_override(tmp_path: Path) -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1, base_directory=tmp_path)
    project.root.add(Color("#ff0000"))
    output = tmp_path / "explicit.mp4"
    result = project.render(output, backend="cpu", overwrite=True)
    assert result.output_path == output
    assert output.is_file()


@pytest.mark.parametrize("backend", ["invalid", 1, object()])
def test_backend_validation_is_centralized(backend: object) -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    with pytest.raises((TypeError, ValueError)):
        project.prepare(backend=backend)  # type: ignore[arg-type]


def test_unregistered_sources_are_rejected_by_central_lowering() -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)
    source = UnknownSource(1)
    layer = project.root.add(source)
    source.value = 9
    assert layer.source is not source
    assert layer.source.value == 1  # type: ignore[attr-defined]
    with pytest.raises(TypeError, match="no lowering registered"):
        project.snapshot()
