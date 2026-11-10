# Shared inputs

All reusable example and benchmark inputs live here. Run commands from the
repository root; generated outputs belong under `target/` or `examples/output/`.
Documentation delivery media stays in `docs/assets/`.

| Directory | Contents and provenance |
| --- | --- |
| `footage/` | Shared video clips, checksums and licenses in [the footage record](footage/README.md). |
| `images/` | The landscape photograph and its separately named study crop; see [image credits](images/README.md). |
| `fonts/` | Manrope and its adjacent [SIL OFL license](fonts/Manrope-OFL.txt); [source and checksum](../examples/showcase/README.md#shared-font). |
| `audio/` | Original deterministic synth, reproduced by `just showcase-assets`. |
| `fixtures/` | Original red, green and blue test images and tone audio used by canonical examples and tests. |

Tutorial programs live in [examples/tutorials](../examples/tutorials/README.md).
Keep licenses beside redistributed inputs and link their records from examples.
Crate-specific test fixtures remain in `tests/assets/`; they are not example inputs.
