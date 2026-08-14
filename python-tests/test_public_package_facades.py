import importlib

import pytest
import vestra
import vestra.authoring
import vestra._native
import vestra.effects as effects
import vestra.properties as properties
import vestra.sources as sources


def test_public_package_facades_preserve_expected_exports() -> None:
    assert vestra.Project is not None
    assert vestra.ProjectSnapshot is not None
    assert vestra.authoring.ProjectBuilder is not None
    assert sources.Image.__module__ == "vestra.sources"
    assert effects.Bloom.__module__ == "vestra.effects"
    assert properties.ScalarProperty.__module__ == "vestra.properties"
    assert importlib.import_module("vestra.sources.image").Image is sources.Image
    assert importlib.import_module("vestra.effects.stylize").Bloom is effects.Bloom
    assert (
        importlib.import_module("vestra.properties.scalar").ScalarProperty
        is properties.ScalarProperty
    )


def test_project_configuration_is_explicitly_construction_time_only() -> None:
    project = vestra.Project(size=(2, 2), fps=1, duration=1)

    assert project.size == (2, 2)
    assert project.duration == 1.0
    with pytest.raises(AttributeError):
        project.size = (4, 4)
    with pytest.raises(AttributeError):
        project.fps = 30
    with pytest.raises(AttributeError):
        project.duration = 2
