"""Generator checks require only Python's standard library, never a renderer."""

from __future__ import annotations

from copy import deepcopy
import importlib.util
import ast
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
        with self.assertRaisesRegex(ValueError, "unknown.*unknown"):
            generator.with_python(data, {"example": {}, "unknown": {}})

    def test_ordinary_effect_needs_no_python_inventory_entry(self):
        data = metadata()
        data["effects"][0].pop("python")
        actual = generator.with_python(data, {})["effects"][0]
        self.assertEqual(actual["python"]["name"], "Example")
        self.assertEqual(actual["python"]["module"], "vestra.effects.stylize")
        self.assertEqual(actual["python"]["collection"]["factory"], "positional")

    def test_optional_interval_before_required_parameters_uses_keyword_arguments(self):
        data = metadata()
        effect = data["effects"][0]
        effect.pop("python")
        interval = dict(
            effect["parameters"][0],
            name="active_interval",
            kind="active_interval",
            required=False,
        )
        effect["parameters"].insert(0, interval)
        effect = generator.with_python(data, {})["effects"][0]
        source = generator.render_class(effect, [effect])
        tree = ast.parse(source)
        constructor = next(
            node
            for node in tree.body[0].body
            if isinstance(node, ast.FunctionDef) and node.name == "__init__"
        )
        self.assertEqual([arg.arg for arg in constructor.args.args], ["self"])
        self.assertEqual(
            ast.unparse(constructor.args.kw_defaults[0]), "ActiveInterval()"
        )
        self.assertIsNone(constructor.args.kw_defaults[1])

    def test_new_effect_reaches_every_public_generation_path(self):
        data = metadata()
        effect = data["effects"][0]
        effect["id"] = "film_grain"
        effect.pop("python")
        effect["parameters"][0].update(
            name="amount", kind="scalar_property", minimum=0, maximum=1
        )
        data["schema"] = data["schema"].replace("example_effect", "film_grain_effect")
        expected = generator.outputs(generator.with_python(data, {}), ROOT)
        high_level = expected[Path("python/vestra/effects/stylize.py")]
        self.assertIn("class FilmGrain(Effect):", high_level)
        self.assertIn(
            "class FilmGrain(Effect):",
            expected[Path("python/vestra/effects/stylize.pyi")],
        )
        advanced = expected[Path("python/vestra/authoring/effects.py")]
        self.assertIn("class FilmGrainEffect(Effect):", advanced)
        self.assertIn("def add_film_grain(", advanced)
        self.assertIn("ModulatableScalarTrack._create(owner, amount)", advanced)
        for facade in [
            "python/vestra/__init__.py",
            "python/vestra/effects/__init__.py",
        ]:
            declarations = ast.parse(expected[Path(facade)]).body
            exports = next(
                node.value
                for node in declarations
                if isinstance(node, ast.Assign)
                and any(
                    isinstance(target, ast.Name) and target.id == "__all__"
                    for target in node.targets
                )
            )
            self.assertIn("FilmGrain", ast.literal_eval(exports))
        self.assertIn(
            "FilmGrainEffect", expected[Path("python/vestra/authoring/__init__.py")]
        )
        for path, source in expected.items():
            if path.suffix in {".py", ".pyi"}:
                compile(source, str(path), "exec")

    def test_new_enum_parameter_generates_its_definition_and_type_association(self):
        data = metadata()
        effect = data["effects"][0]
        effect.pop("python")
        effect["parameters"][0].update(
            name="mode", kind="enum", default="soft", enum_values=["soft", "hard"]
        )
        expected = generator.outputs(generator.with_python(data, {}), ROOT)
        self.assertIn(
            "class ExampleMode(Enum):",
            expected[Path("python/vestra/authoring/effects.py")],
        )
        self.assertIn(
            "SOFT = 'soft'", expected[Path("python/vestra/authoring/effects.py")]
        )
        self.assertIn(
            "('example', 'mode'): ExampleMode",
            expected[Path("python/vestra/effects/base.py")],
        )
        self.assertIn("ExampleMode", expected[Path("python/vestra/effects/stylize.py")])

    def test_python_module_override_creates_sources_without_a_handwritten_shell(self):
        data = metadata()
        data["effects"][0].pop("python")
        expected = generator.outputs(
            generator.with_python(data, {"example": {"module": "custom"}}), ROOT
        )
        for extension in [".py", ".pyi"]:
            self.assertIn(
                "class Example(Effect):",
                expected[Path("python/vestra/effects/custom" + extension)],
            )
        self.assertIn(
            "from .custom import (", expected[Path("python/vestra/effects/__init__.py")]
        )

    def test_duplicate_python_enum_members_fail_before_writing(self):
        data = metadata()
        effect = data["effects"][0]
        effect["parameters"][0].update(
            kind="enum", default="one", enum_values=["one", "ONE"]
        )
        effect["python"].update(enums={"edge_characters": "ExampleMode"}, defaults={})
        with self.assertRaisesRegex(ValueError, "ExampleMode.*duplicate"):
            generator.validate(data)

    def test_shared_python_enum_requires_identical_ordered_values(self):
        data = metadata()
        effect = data["effects"][0]
        effect["parameters"][0].update(
            kind="enum", default="one", enum_values=["one", "two"]
        )
        effect["python"].update(enums={"edge_characters": "SharedMode"}, defaults={})
        other = deepcopy(effect)
        other["id"] = "other"
        other["python"]["name"] = "Other"
        other["parameters"][0]["enum_values"] = ["two", "one"]
        data["effects"].append(other)
        data["schema"] = (
            json.dumps(
                {
                    "$defs": {
                        "effect": {
                            "oneOf": [
                                {"$ref": "#/$defs/example_effect"},
                                {"$ref": "#/$defs/other_effect"},
                            ]
                        }
                    }
                }
            )
            + "\n"
        )
        with self.assertRaisesRegex(ValueError, "SharedMode.*inconsistent"):
            generator.validate(data)

    def test_every_artifact_family_detects_drift_without_rewriting(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            module = root / "python/vestra/effects/example.py"
            module.parent.mkdir(parents=True)
            module.write_text(
                generator.IMPORT_BEGIN
                + "\n"
                + generator.IMPORT_END
                + "\n"
                + generator.BEGIN
                + "\n"
                + generator.END
                + "\n"
            )
            authoring = root / "python/vestra/authoring/effects.py"
            authoring.parent.mkdir(parents=True)
            authoring.write_text(
                generator.ENUM_BEGIN
                + "\n"
                + generator.ENUM_END
                + "\n"
                + generator.ADVANCED_BEGIN
                + "\n"
                + generator.ADVANCED_END
                + "\n"
                + "class Collection:\n"
                + generator.HELPER_BEGIN
                + "\n"
                + generator.HELPER_END
                + "\nclass ClipCollection:\n"
                + generator.CLIP_BEGIN
                + "\n"
                + generator.CLIP_END
                + "\n"
            )
            for path in [
                "python/vestra/__init__.py",
                "python/vestra/effects/__init__.py",
                "python/vestra/authoring/__init__.py",
            ]:
                (root / path).write_text(
                    generator.IMPORT_BEGIN
                    + "\n"
                    + generator.IMPORT_END
                    + "\n__all__ = []\n"
                )
            (root / "python/vestra/effects/base.py").write_text(
                generator.IMPORT_BEGIN
                + "\n"
                + generator.IMPORT_END
                + "\n"
                + generator.ENUM_TYPES_BEGIN
                + "\n"
                + generator.ENUM_TYPES_END
                + "\n"
            )
            expected = generator.outputs(metadata(), root)
            generator.synchronize(expected, root, check=False)
            self.assertEqual(generator.outputs(metadata(), root), expected)
            self.assertEqual(generator.synchronize(expected, root, check=True), [])
            for path, content in expected.items():
                with self.subTest(path=str(path)):
                    stale = root / path
                    if path.suffix == ".py":
                        marker = next(
                            marker
                            for marker in [
                                generator.BEGIN,
                                generator.HELPER_BEGIN,
                                generator.IMPORT_BEGIN,
                            ]
                            if marker in content
                        )
                        changed = content.replace(
                            marker + "\n", marker + "\n# stale generated content\n", 1
                        )
                    else:
                        changed = content + "\n"
                    stale.write_text(changed)
                    modified = stale.stat().st_mtime_ns
                    regenerated = generator.outputs(metadata(), root)
                    self.assertEqual(
                        generator.synchronize(regenerated, root, check=True), [path]
                    )
                    self.assertEqual(stale.read_text(), changed)
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
