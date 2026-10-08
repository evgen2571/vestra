---
name: api-contract-sync
description: "Use when changing Vestra canonical JSON models or schemas, effect descriptors, Rust SDK exports, Python/PyO3 APIs, public typing, validation, or authoring/lowering behavior."
---

# Synchronize Rust, JSON and Python contracts

Read [project-to-plan](../../../docs/development/architecture/project-to-plan.md),
[Python bindings](../../../docs/development/architecture/python-bindings.md),
[effect reference](../../../docs/reference/effects.md)
and [testing](../../../docs/development/testing.md).

## Contract map

| Layer | Starting point |
| --- | --- |
| Canonical tagged model | `crates/vestra-core/src/project/model/` |
| Effect metadata and constraints | `crates/vestra-core/src/effect_definition.rs` |
| Validation, compilation, evaluation | `crates/vestra-core/src/validation/`, `src/plan/` |
| JSON schema | `schemas/project.schema.json` and `scripts/check-schema.sh` |
| Advanced authoring | `python/vestra/authoring/` |
| High-level Python | `python/vestra/effects/`, `python/vestra/lowering.py` |
| Native binding and typing | `crates/vestra-python/` and relevant `.pyi` files |
| Rust public SDK | `crates/vestra/` |
| Public docs | `docs/reference/` and `docs/guides/python/` |

## Procedure

1. Specify the canonical tag, required/optional fields, type/range, units,
   defaults, scope and diagnostics before writing public constructors.
2. Change model and descriptor metadata, semantic validation and
   core compile/evaluate logic together. Do not duplicate authored
   defaults or special cases in each frontend.
3. Update high-level and advanced Python paths, exports, lowering and
   static typing. Touch PyO3 and native stubs only when binding interfaces
   actually change.
4. Regenerate and validate the canonical schema, test positive/negative
   JSON, round trips, high-level API, Rust SDK and typing failure cases.
5. Update [reference](../../../docs/reference/effects.md),
   [support matrix](../../../docs/reference/feature-support.md) and
   runnable examples. Serialization alone is not evidence of renderer support.

## Verification

Run targeted Rust/Python tests while editing. Use `just schema-check`,
`just python-test` and `just check` when the native toolchain is available.
Distinguish intentional compatibility changes from accidental schema or
API drift and report unverified interfaces explicitly.
