"""Keep the downloadable introduction identical to its canonical showcase."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_animation_download_matches_imported_quickstart() -> None:
    assert (ROOT / "docs/assets/examples/first_animation.py").read_bytes() == (
        ROOT / "examples/showcase/quickstart/main.py"
    ).read_bytes()


def test_python_snippets_have_valid_syntax() -> None:
    """Catch broken edits in examples even though the site only highlights them."""
    import ast
    import re
    import textwrap

    for page in (ROOT / "docs").rglob("*.md"):
        if {"agents", "plans"} & set(page.parts):
            continue
        for block in re.finditer(
            r"^ *```python[^\n]*\n(.*?)^ *```",
            page.read_text(),
            re.MULTILINE | re.DOTALL,
        ):
            code = textwrap.dedent(block[1])
            for include in re.finditer(r'^ *--8<-- "([^"]+)"$', code, re.MULTILINE):
                source = (ROOT / include[1]).read_text()
                code = code.replace(include[0], source)
            ast.parse(code, filename=str(page))
