# Frozen effect compatibility fixtures

These expected values were captured before descriptor-driven generation at
`b23c985a623efea14511e25aeb1df93b495beecb`, with a freshly rebuilt native extension
reporting version `0.1.3`. They are inputs to
`python-tests/test_effect_generation_compatibility.py`; the tests do not read
temporary baseline logs or run a fixture-update command.

- `python-api.json` records exports, constructors, annotations, collection
  helpers, source imports and class structure.
- `python-behavior.json` records canonical serialization, animation, numerical
  boundaries, errors and validation timing.
- `json-v1.json` records schema acceptance, Rust parsing, semantic diagnostics,
  round trips and the exact schema checksum for 266 cases.

The expected catalog has 23 canonical effects, 24 Python constructors including
the `PseudoAscii` adapter, and 21 typed advanced helpers. The generic advanced
path covers all 23 effects. Ascii omissions of `glyph_style`, `mode` and
`color_mode` intentionally pass the schema and fail Rust parsing.

To check the current source, run from the repository root in its development
environment:

```bash
just python-test python-tests/test_effect_generation_compatibility.py
```

To verify the provenance independently, extract the exact baseline into a fresh
directory and copy only this test and its three frozen JSON inputs there. The
test imports existing baseline test helpers, so retain the complete baseline
source tree:

```bash
effect_baseline=$(mktemp -d)
git archive b23c985a623efea14511e25aeb1df93b495beecb | tar -x -C "$effect_baseline"
cp python-tests/test_effect_generation_compatibility.py "$effect_baseline/python-tests/"
mkdir -p "$effect_baseline/python-tests/fixtures/effects"
cp python-tests/fixtures/effects/*.json "$effect_baseline/python-tests/fixtures/effects/"
cd "$effect_baseline"
nix develop --command bash -euc '
  uv sync --locked --extra dev --reinstall-package vestra
  uv run python -c "import vestra; print(vestra.native_version())"
  uv run pytest python-tests/test_effect_generation_compatibility.py
'
```

Use an equivalent native setup if Nix is unavailable. Rebuild the extension for
each source revision; an editable Python checkout with an old native binary is
insufficient evidence. The separate directory keeps the current branch and its
generated files intact. Compare failures with the recorded baseline before
changing expectations. Updating these fixtures requires an intentional public
contract change and independently reviewed expected values, rather than a
snapshot of the generator's current output.
