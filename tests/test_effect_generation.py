"""Generator checks require only Python's standard library, never a renderer."""

from __future__ import annotations

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import re
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "generate_effects", ROOT / "scripts/generate-effects.py"
)
generator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(generator)


def metadata():
    parameter = {
        "name": "edge_characters",
        "kind": "string",
        "required": True,
        "minimum": None,
        "maximum": None,
        "integer_minimum": None,
        "integer_maximum": None,
        "minimum_exclusive": False,
        "maximum_exclusive": False,
        "default": None,
        "enum_values": [],
    }
    effect = {
        "id": "example",
        "scope": "clip_and_global",
        "class": "advanced",
        "parameters": [parameter],
        "stage": "post_transform",
        "estimated_pass_count": 1,
        "retains_original": False,
        "temporal_policy": "from_properties",
        "units": [],
        "python": {
            "name": "Example",
            "module": "vestra.effects.example",
            "base": "Effect",
            "doc": None,
            "collection": {"factory": "palette", "parameter_order": []},
            "positional": 0,
            "defaults": {"edge_characters": "a|b"},
        },
    }
    return {
        "effects": [effect],
        "schema": json.dumps(
            {"$defs": {"effect": {"oneOf": [{"$ref": "#/$defs/example_effect"}]}}}
        )
        + "\n",
    }


def many_parameters():
    data = metadata()
    effect = data["effects"][0]
    effect["parameters"] = [
        dict(effect["parameters"][0], name=f"parameter_{i}") for i in range(12)
    ]
    effect["python"]["defaults"] = {f"parameter_{i}": "value" for i in range(12)}
    return data


class EffectGenerationTests(unittest.TestCase):
    def test_reference_escapes_pipe_in_python_default(self):
        text = generator.render_reference(metadata()["effects"])
        row = next(
            line for line in text.splitlines() if line.startswith("| `edge_characters`")
        )
        self.assertIn(r"a\|b", row)
        self.assertEqual(len(re.split(r"(?<!\\)\|", row)), 9)

    def test_generated_constructor_mapping_is_readable(self):
        data = many_parameters()
        text = generator.render_class(data["effects"][0], data["effects"])
        self.assertLessEqual(max(map(len, text.splitlines())), 120)
        compile(text, "generated.py", "exec")

    def test_generated_helper_mapping_is_readable(self):
        text = generator.render_helper(many_parameters()["effects"][0])
        self.assertLessEqual(max(map(len, text.splitlines())), 120)

    def test_stub_constructor_remains_readable(self):
        data = many_parameters()
        source = generator.render_class(data["effects"][0], data["effects"])
        text = generator.render_stub(source)
        self.assertLessEqual(max(map(len, text.splitlines())), 120)
        compile(text, "generated.pyi", "exec")

    def test_regeneration_is_idempotent_and_check_does_not_write(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = Path("generated.py")
            expected = {path: "# generated\n"}
            self.assertEqual(generator.synchronize(expected, root, check=True), [path])
            self.assertFalse((root / path).exists())
            self.assertEqual(generator.synchronize(expected, root, check=False), [path])
            stat = (root / path).stat()
            self.assertEqual(generator.synchronize(expected, root, check=False), [])
            self.assertEqual((root / path).stat().st_mtime_ns, stat.st_mtime_ns)
            different = {path: "# changed\n"}
            self.assertEqual(generator.synchronize(different, root, check=True), [path])
            self.assertEqual((root / path).read_text(), expected[path])
            self.assertEqual((root / path).stat().st_mtime_ns, stat.st_mtime_ns)

    def test_canonical_kind_and_default_propagate_without_signature_copies(self):
        data = metadata()
        effect = data["effects"][0]
        effect["python"].pop("defaults")
        parameter = effect["parameters"][0]
        parameter.update(
            name="levels",
            kind="integer",
            required=False,
            default=4,
            integer_minimum=2,
            integer_maximum=256,
        )
        generator.validate(data)
        before = generator.render_class(effect, [effect])
        stub_before = generator.render_stub(before)
        reference_before = generator.render_reference([effect])
        self.assertIn("levels: int = 4", before)
        self.assertIn("levels: int = 4", stub_before)
        self.assertIn("`4`", reference_before)
        parameter["default"] = 8
        after = generator.render_class(effect, [effect])
        self.assertIn("levels: int = 8", after)
        self.assertIn("levels: int = 8", generator.render_stub(after))
        self.assertIn("levels: int = 8", generator.render_helper(effect))
        self.assertIn("optional; `8`", generator.render_reference([effect]))
        parameter.update(kind="number", minimum=2, maximum=256)
        self.assertIn(
            "levels: int | float = 8", generator.render_class(effect, [effect])
        )

    def test_explicit_static_getter_fallback_is_preserved(self):
        data = metadata()
        effect = data["effects"][0]
        effect["python"]["get_values"] = True
        source = generator.render_class(effect, [effect])
        self.assertIn("self._values.get('edge_characters')", source)

    def test_invalid_python_enum_override_fails_before_generation(self):
        data = metadata()
        effect = data["effects"][0]
        effect["parameters"][0].update(
            kind="enum", enum_values=["valid"], default="valid"
        )
        effect["python"].update(enums={"edge_characters": "ExampleMode"}, defaults={})
        generator.validate(data)
        effect["python"]["defaults"]["edge_characters"] = "missing"
        with self.assertRaisesRegex(ValueError, "invalid Python enum/default"):
            generator.validate(data)

    def test_python_overrides_must_match_the_canonical_inventory(self):
        data = metadata()
        with self.assertRaisesRegex(ValueError, "must cover the catalog"):
            generator.with_python(data, {})
        with self.assertRaisesRegex(ValueError, "must cover the catalog"):
            generator.with_python(data, {"example": {}, "unknown": {}})

    def test_every_artifact_family_detects_drift_without_rewriting(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            module = root / "python/vestra/effects/example.py"
            module.parent.mkdir(parents=True)
            module.write_text(generator.BEGIN + "\n" + generator.END + "\n")
            authoring = root / "python/vestra/authoring/effects.py"
            authoring.parent.mkdir(parents=True)
            authoring.write_text(
                "class Collection:\n"
                + generator.HELPER_BEGIN
                + "\n"
                + generator.HELPER_END
                + "\nclass ClipCollection:\n"
                + generator.CLIP_BEGIN
                + "\n"
                + generator.CLIP_END
                + "\n"
            )
            expected = generator.outputs(metadata(), root)
            generator.synchronize(expected, root, check=False)
            self.assertEqual(generator.outputs(metadata(), root), expected)
            self.assertEqual(generator.synchronize(expected, root, check=True), [])
            for path, content in expected.items():
                with self.subTest(path=str(path)):
                    stale = root / path
                    stale.write_text(content + "\n")
                    modified = stale.stat().st_mtime_ns
                    self.assertEqual(
                        generator.synchronize(expected, root, check=True), [path]
                    )
                    self.assertEqual(stale.read_text(), content + "\n")
                    self.assertEqual(stale.stat().st_mtime_ns, modified)
                    stale.write_text(content)

    def test_generated_sections_preserve_handwritten_adapters(self):
        text = "adapter_before\nBEGIN\nold\nEND\nadapter_after\n"
        actual = generator.replace_section(text, "BEGIN", "END", "new\n")
        self.assertEqual(actual, "adapter_before\nBEGIN\nnew\nEND\nadapter_after\n")
        with self.assertRaisesRegex(ValueError, "expected one generated section"):
            generator.replace_section("no markers", "BEGIN", "END", "new")

    def test_invalid_metadata_has_actionable_diagnostics(self):
        mutations = [
            (
                lambda data: data["effects"].append(deepcopy(data["effects"][0])),
                "duplicate effect",
            ),
            (
                lambda data: data["effects"][0]["parameters"].clear(),
                "unknown parameter",
            ),
            (
                lambda data: data["effects"][0]["parameters"][0].update(
                    kind="unsupported"
                ),
                "unsupported kind",
            ),
            (
                lambda data: data["effects"][0]["parameters"][0].update(
                    minimum=2, maximum=1
                ),
                "inverted bounds",
            ),
            (
                lambda data: data["effects"][0]["python"].update(order=["missing"]),
                "constructor order",
            ),
            (
                lambda data: data["effects"][0]["python"].update(positional=2),
                "positional argument count",
            ),
            (
                lambda data: data["effects"][0]["python"]["collection"].update(
                    factory="unknown"
                ),
                "unsupported collection factory",
            ),
            (
                lambda data: data["effects"][0]["python"]["collection"].update(
                    parameter_order=["absent"]
                ),
                "helper order",
            ),
            (
                lambda data: data["effects"][0].update(units=[["absent", "px"]]),
                "unknown parameter",
            ),
            (
                lambda data: data.update(
                    schema=data["schema"].replace("example_effect", "absent_effect")
                ),
                "schema effect branches",
            ),
        ]
        generator.validate(metadata())
        for mutate, diagnostic in mutations:
            with self.subTest(diagnostic=diagnostic):
                data = metadata()
                mutate(data)
                with self.assertRaisesRegex(ValueError, diagnostic):
                    generator.validate(data)


if __name__ == "__main__":
    unittest.main()
