# Testing

The canonical local check is `./scripts/check.sh`. It runs Rust formatting, workspace check, Clippy, workspace tests, Python schema validation, and schema regeneration comparison. CI runs those categories in Nix development shells, plus Python package tests/build smoke and software-WGPU correctness.

Use `uv sync --locked --extra dev` for Python tooling. Run focused Rust/Python tests while changing a contract, then the relevant broader check. WGPU software and hardware validation are distinct; see [GPU validation](gpu-validation.md). Native media builds need FFmpeg executables and development libraries available to the selected environment.
