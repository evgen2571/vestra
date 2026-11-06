"""Check the catalog's controlled inputs and its order-dependent comparison."""

from pathlib import Path
import runpy

ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = runpy.run_path(str(ROOT / "docs/assets/examples/effects_catalog.py"))


def test_every_catalog_effect_authors_a_valid_project():
    for name in EXAMPLE["effect_cases"]():
        project = EXAMPLE["make_project"](name)
        report = project.validate()
        assert report.is_valid, (name, report.diagnostics)


def test_order_comparison_keeps_inputs_but_changes_the_result():
    first = EXAMPLE["make_project"]("order")
    second = EXAMPLE["make_project"]("order", reverse=True)
    assert first.validate().is_valid
    assert second.validate().is_valid
    a = first.snapshot().to_dict()["visual"]["clips"][0]
    b = second.snapshot().to_dict()["visual"]["clips"][0]
    assert a["source"] == b["source"]
    assert [effect["type"] for effect in a["effects"]] == [
        "gaussian_blur",
        "ordered_dither",
    ]
    assert [effect["type"] for effect in b["effects"]] == [
        "ordered_dither",
        "gaussian_blur",
    ]
    assert (
        first.render_frame(0.7, backend="cpu").to_bytes()
        != second.render_frame(0.7, backend="cpu").to_bytes()
    )


def test_catalog_settings_match_the_runnable_constructors():
    import ast
    import re

    tree = ast.parse((ROOT / "docs/assets/examples/effects_catalog.py").read_text())
    cases = next(
        node
        for node in tree.body
        if isinstance(node, ast.FunctionDef) and node.name == "effect_cases"
    )
    values = next(
        node.value for node in cases.body if isinstance(node, ast.Return)
    ).values
    expected = [ast.dump(value) for value in values if isinstance(value, ast.Call)]
    page = (ROOT / "docs/guides/python/effect-catalog.md").read_text()
    actual = [
        ast.dump(ast.parse(code.strip(), mode="eval").body)
        for code in re.findall(r"```python\n(.*?)```", page, re.DOTALL)
    ]
    assert actual == expected


def test_catalog_represents_every_public_canonical_effect():
    from vestra.effects import available_effects

    represented = {
        effect.type
        for effect in EXAMPLE["effect_cases"]().values()
        if effect is not None
    }
    assert represented == {descriptor["id"] for descriptor in available_effects()}


def test_temporal_comparisons_show_both_rows_and_tile_fills_its_viewport():
    for name in ("motion-tile", "motion-blur", "camera-shake"):
        project = EXAMPLE["comparison_project"](name)
        assert project.validate().is_valid
        assert project.output_audio is False
        prepared = project.prepare(backend="cpu")
        for seconds in (0.2, 0.7, 1.4, 2.6):
            frame = prepared.render_frame_seconds(seconds)
            assert (frame.width, frame.height) == (640, 520)
            pixels = frame.to_bytes()
            # Both rows retain the scene at matching timestamps, rather than a reel.
            top = pixels[40 * 640 * 4 : 260 * 640 * 4]
            bottom = pixels[300 * 640 * 4 : 520 * 640 * 4]
            assert top != bottom, (name, seconds)
            if name == "motion-tile":
                backdrop = bytes.fromhex("1a242b")
                uncovered = sum(
                    bottom[i : i + 3] == backdrop for i in range(0, len(bottom), 4)
                )
                assert uncovered == 0, (seconds, uncovered)
