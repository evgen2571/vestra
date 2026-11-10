"""Font resolution contracts, independent of any particular host font."""

from pathlib import Path
import shutil
import struct
import subprocess
import sys

import pytest

import vestra
from vestra import Text
from vestra import _fonts, _native


ROOT = Path(__file__).resolve().parents[1]
FONT = ROOT / "tests/assets/VestraTest-Regular.ttf"


def project_with(font=None, *, base_directory="."):
    project = vestra.Project(size=(320, 120), fps=1, duration=1, base_directory=base_directory)
    project.root.add(Text("Hello AV ffi Δ Привет", font=font, font_size=32))
    return project


def test_default_is_optional_and_none_uses_same_packaged_asset(monkeypatch):
    def no_system_fonts(*args):
        pytest.fail("the default font must not scan or resolve system fonts")
    monkeypatch.setattr(_native, "_resolve_system_font", no_system_fonts)
    project = project_with()
    project.root.add(Text("Default"))
    data = project.snapshot().to_dict()
    assert len(data["assets"]) == 1
    assert data["visual"]["clips"][1]["source"]["font_size"] == 48
    path = Path(data["assets"][0]["source"])
    assert path.is_file() and path.name == "VestraDefault.ttf"
    assert path.with_name("LICENSE.txt").is_file()
    pixels = project.render_frame(0, backend="cpu").to_bytes()
    assert any(pixels[index:index + 3] != b"\0\0\0" for index in range(0, len(pixels), 4))


@pytest.mark.skipif(sys.platform != "linux", reason="fontconfig isolation is Linux-specific")
def test_default_renders_in_fresh_process_with_empty_fontconfig(tmp_path):
    config = tmp_path / "fonts.conf"
    empty = tmp_path / "empty-fonts"
    empty.mkdir()
    config.write_text(f'<?xml version="1.0"?><fontconfig><dir>{empty}</dir></fontconfig>')
    import os
    env = dict(os.environ, FONTCONFIG_FILE=str(config))
    subprocess.run([sys.executable, "-c", """
import vestra
assert vestra.list_system_fonts() == []
p = vestra.Project(size=(160, 80), fps=1, duration=1)
p.root.add(vestra.Text('Hello', font_size=32))
b = p.render_frame(0, backend='cpu').to_bytes()
assert any(b[i:i+3] != bytes(3) for i in range(0, len(b), 4))
"""], cwd=tmp_path, env=env, check=True)


def test_family_resolves_during_snapshot_and_deduplicates_with_file(monkeypatch):
    calls = []
    def resolve(family):
        calls.append(family)
        return FONT, 0
    monkeypatch.setattr(_native, "_resolve_system_font", resolve)
    project = project_with("Test Family")
    project.root.add(Text("file", font=FONT, font_size=24))
    data = project.snapshot().to_dict()
    assert calls == ["Test Family"]
    assert len(data["assets"]) == 1
    assert data["assets"][0]["source"] == str(FONT)
    pixels = project.render_frame(0, backend="cpu").to_bytes()
    assert any(pixels[i:i + 3] != bytes(3) for i in range(0, len(pixels), 4))
    snapshot = project.snapshot()
    assert vestra.ProjectSnapshot.from_json(snapshot.to_json()).to_dict() == snapshot.to_dict()


def test_unknown_family_has_no_fallback():
    with pytest.raises(ValueError, match="system font family.*unavailable.*list_system_fonts"):
        project_with("Vestra Definitely Missing Family 937").snapshot()


@pytest.mark.parametrize("value", ["Regular.ttf", "Regular.otf", "custom.font", "sub/font", Path("extensionless")])
def test_explicit_path_spelling_is_preserved(value, tmp_path, monkeypatch):
    def unexpected(*args):
        pytest.fail("explicit paths must not use family discovery")
    monkeypatch.setattr(_native, "_resolve_system_font", unexpected)
    project = project_with(value, base_directory=tmp_path)
    assert project.snapshot().to_dict()["assets"][0]["source"] == str(value)


def test_existing_extensionless_relative_file_uses_project_base(tmp_path, monkeypatch):
    shutil.copyfile(FONT, tmp_path / "custom")
    project = project_with("custom", base_directory=tmp_path)
    assert project.snapshot().to_dict()["assets"][0]["source"] == "custom"
    pixels = project.render_frame(0, backend="cpu").to_bytes()
    assert any(pixels[i:i + 3] != bytes(3) for i in range(0, len(pixels), 4))


@pytest.mark.parametrize("suffix", ["ttf", "otf"])
def test_absolute_file_and_pathlike_render(tmp_path, suffix):
    path = tmp_path / f"Font.{suffix}"
    shutil.copyfile(FONT, path)
    assert any(project_with(path).render_frame(0, backend="cpu").to_bytes())
    assert project_with(str(path)).snapshot().to_dict()["assets"][0]["source"] == str(path)


def test_missing_path_fails_during_preparation(tmp_path):
    with pytest.raises(vestra.PreparationError, match="cannot access"):
        project_with(tmp_path / "missing.ttf").render_frame(0, backend="cpu")


def _collection(*fonts: bytes) -> bytes:
    """Pack real sfnt fixtures into a collection with absolute table offsets."""
    output = bytearray(struct.pack(">4sII", b"ttcf", 0x10000, len(fonts)) + bytes(4 * len(fonts)))
    for index, font in enumerate(fonts):
        offset = len(output)
        struct.pack_into(">I", output, 12 + index * 4, offset)
        rebased = bytearray(font)
        count = struct.unpack_from(">H", rebased, 4)[0]
        for table in range(count):
            position = 12 + table * 16 + 8
            start = struct.unpack_from(">I", rebased, position)[0]
            struct.pack_into(">I", rebased, position, start + offset)
        output.extend(rebased)
        output.extend(b"\0" * (-len(output) % 4))
    return bytes(output)


def test_collection_uses_intended_nonzero_face_and_persistent_asset(tmp_path, monkeypatch):
    other = ROOT / "assets/fonts/Manrope.ttf"
    collection = tmp_path / "fonts.ttc"
    collection.write_bytes(_collection(FONT.read_bytes(), other.read_bytes()))
    monkeypatch.setenv("XDG_CACHE_HOME", str(tmp_path / "cache"))
    monkeypatch.setattr(_fonts, "_cache_directory", lambda: tmp_path / "cache/vestra/fonts")
    monkeypatch.setattr(_native, "_resolve_system_font", lambda family: (collection, 1))
    project = project_with("Selected Family")
    snapshot = project.snapshot()
    resolved = Path(snapshot.to_dict()["assets"][0]["source"])
    assert resolved.is_file() and resolved.read_bytes()[:4] != b"ttcf"
    assert _fonts._checksum(resolved.read_bytes()) == 0xB1B0AFBA
    assert project.render_frame(0, backend="cpu").to_bytes() == project_with(other).render_frame(0, backend="cpu").to_bytes()
    assert project.snapshot().to_dict() == snapshot.to_dict()
    assert len(list(resolved.parent.iterdir())) == 1
    # Materialized assets survive a fresh interpreter; no collection state is required.
    subprocess.run([sys.executable, "-c", """
import sys, vestra
p = vestra.ProjectSnapshot.from_json(sys.argv[1])
e = vestra.Editor().prepare(p, vestra.PrepareOptions(backend=vestra.BackendPreference.CPU))
assert any(e.render_frame_number(0).to_bytes())
""", snapshot.to_json()], cwd=tmp_path, check=True)


@pytest.mark.parametrize("data,index", [(b"ttcf", 0), (_collection(FONT.read_bytes()), 1), (b"garbage", 0)])
def test_invalid_collection_is_rejected(data, index):
    with pytest.raises(ValueError, match="collection"):
        _fonts._extract_face(data, index)


def test_system_discovery_and_real_family_smoke():
    families = vestra.list_system_fonts()
    assert families == sorted(families, key=lambda name: (name.lower(), name))
    assert len({name.lower() for name in families}) == len(families)
    assert families == vestra.list_system_fonts()
    if not families:
        pytest.skip("no host fonts discovered; empty discovery tested separately")
    # Prefer a Latin family if available; no particular font is required.
    family = next((name for name in families if "sans" in name.lower()), families[0])
    path, index = _native._resolve_system_font(family)
    assert _native._resolve_system_font(family.swapcase()) == (path, index)
    assert Path(path).is_file()
    assert any(project_with(family).render_frame(0, backend="cpu").to_bytes())


def test_default_text_in_nested_composition_and_mask_round_trip():
    project = vestra.Project(size=(160, 80), fps=1, duration=1)
    group = project.root.group("nested")
    text = group.add(Text("Nested", font_size=24))
    text.masks.add(Text("Mask", font_size=24))
    data = project.snapshot().to_dict()
    assert len(data["assets"]) == 1
    assert vestra.Editor().validate(vestra.ProjectSnapshot.from_dict(data)).is_valid
    pixels = project.render_frame(0, backend="cpu").to_bytes()
    assert any(pixels[i:i + 3] != bytes(3) for i in range(0, len(pixels), 4))


@pytest.mark.skipif(sys.platform != "linux", reason="fontconfig isolation is Linux-specific")
def test_fontconfig_configured_directory_discovers_real_font(tmp_path):
    directory = tmp_path / "configured-fonts"
    directory.mkdir()
    copied = directory / "Regular.ttf"
    shutil.copyfile(FONT, copied)
    config = tmp_path / "fonts.conf"
    config.write_text(f'<?xml version="1.0"?><fontconfig><dir>{directory}</dir></fontconfig>')
    import os
    subprocess.run([sys.executable, "-c", """
from pathlib import Path
import sys, vestra
from vestra import _native
families = vestra.list_system_fonts()
assert families
path, index = _native._resolve_system_font(families[0].swapcase())
assert Path(path) == Path(sys.argv[1]) and index == 0
p = vestra.Project(size=(160, 80), fps=1, duration=1)
p.root.add(vestra.Text('Hello', font=families[0], font_size=32))
pixels = p.render_frame(0, backend='cpu').to_bytes()
assert any(pixels[i:i+3] != bytes(3) for i in range(0, len(pixels), 4))
""", str(copied)], cwd=tmp_path, env=dict(os.environ, FONTCONFIG_FILE=str(config)), check=True)


@pytest.mark.parametrize("font", [None, "Fixture Family"])
def test_font_selection_and_transform_defaults_render_inside_standalone_group(font, monkeypatch):
    monkeypatch.setattr(_native, "_resolve_system_font", lambda family: (FONT, 0))
    source = Text("Hello Δ", font=font, font_size=24,
                  position=(0.5, 0.5), scale=0.8, rotation_degrees=12)
    reference = vestra.Project(size=(160, 80), fps=1, duration=1)
    reference.root.add(source)
    grouped = vestra.Project(size=(160, 80), fps=1, duration=1)
    grouped.root.add(vestra.Group([source]))
    grouped.root.add(vestra.Group([source]), visible=False)
    assert len(grouped.snapshot().to_dict()["assets"]) == 1
    pixels = grouped.render_frame(0, backend="cpu").to_bytes()
    assert any(pixels[i:i + 3] != bytes(3) for i in range(0, len(pixels), 4))
    assert pixels == reference.render_frame(0, backend="cpu").to_bytes()
