# Effect test reference

Use this page when changing effect declarations, generation or registration.
Start with `just effect-check`; it runs generation freshness, core/common/CPU
tests and adapter-free WGPU tests. Add effect-specific pixel regressions and
[adapter/output checks](gpu-validation.md#understand-the-test-evidence) for the
rendering paths you change. The [effect extension guide](extending/effect.md)
describes implementation order.

## Generation and Python compatibility

`just gen-effects-check` builds only the core metadata exporter and uses Python's
standard library. It checks committed-artifact freshness and runs exporter and
generator tests without building renderers or the native extension.

To run the generator tests separately, then check frozen public contracts:

```bash
uv run --no-project python -m unittest discover -s tests -p test_effect_generation.py
uv sync --locked --extra dev --reinstall-package vestra
just python-test python-tests/test_effect_generation_compatibility.py
```

The compatibility test compares imports, signatures, inheritance, serialization,
validation timing and JSON behavior with frozen pre-generation fixtures. It
preserves the Ascii schema/parser mismatch: omitted `glyph_style`, `mode` and
`color_mode` pass the schema but fail Rust parsing. The
[fixture README](https://github.com/evgen2571/vestra/blob/develop/python-tests/fixtures/effects/README.md)
records provenance and the procedure for repeating these comparisons against
the exact baseline revision.

**Keep expectations independent of generated output.** An intentional public
contract change needs reviewed expected values; a generator snapshot cannot
establish compatibility. When adding a public effect, update tests that pin
catalog counts while retaining existing specimens and testing the new behavior.

## Find structural and renderer coverage

| Contract | Tests and specimens |
| --- | --- |
| Catalog visitors, compiled constraints, negative scalar/plain/point keyframes and active intervals | [Core compatibility tests](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-core/src/plan/compiler/effects/compatibility_tests.rs) and [validation tests](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-core/src/validation/effects/tests.rs) |
| Serde, compilation, evaluation, topology, transition remapping and normalization | Frozen specimens in the core compatibility tests |
| Conservative topology and reachable operations | [Common kernel conformance tests](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-render-common/src/kernel.rs) |
| Kernel inventory, order and private bits | Common kernel tests |
| CPU algorithms and pixel behavior | [CPU effect tests](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-render-cpu/src/cpu/effects/tests.rs) |
| Shader labels and ordered composed bytes | [WGPU pipeline tests](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-render-wgpu/src/wgpu/pipeline.rs) |
| Packed-value specimens, record layout and typed uploads | [WGPU parameter tests](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-render-wgpu/src/wgpu/parameters/effect_registration_tests.rs) and [effect parameters](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-render-wgpu/src/wgpu/parameters/effects.rs) |

Registry-driven WGPU checks validate every registered record against WGSL,
including size/alignment, named field offsets, scalar types and typed uploads.
They do not prove every packed value or the algorithm. Frozen specimens preserve
selected behavior; structural tests do not establish exhaustive numerical
correctness, rendered quality or hardware parity.

A public effect needs a specimen in the common kernel conformance table. A new
kernel also needs rows in the kernel-order, shader-byte and packed-record tests;
keep existing rows unchanged. See [feature isolation](testing.md#backend-feature-isolation)
when changing crate or backend dependencies.
