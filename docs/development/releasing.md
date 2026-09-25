# Releasing the Python package

The workspace version in `Cargo.toml` is the release version for every Rust
crate. Maturin reads the Python package version from `vestra-python` through
`pyproject.toml`'s dynamic version setting. Project JSON and RenderEvent both
use schema version 1 for the first public `0.1.0` release.

Run `./scripts/check.sh` and the Python tests before building release artifacts.
Changing the release workflow in a pull request runs its artifact matrix;
after merge, it can also be started manually. The workflow builds and smoke
tests CPython 3.11 through 3.14 wheels on Linux x86_64 and aarch64, macOS Intel
and Apple Silicon, and Windows x64. The workflow also builds a source
distribution and installs it in a clean environment. Each install must pass
`scripts/verify-wheel.py`, which checks the installed version, creates and
validates a project, renders an MP4, and checks its AAC audio. Review every
matrix job and its uploaded artifact before publishing.

Wheels statically link a bundled FFmpeg build. The local patch under
`vendor/ffmpeg-sys-next` removes the dependency's host-specific
`-march=native` compiler flag and fixes FFmpeg's MSVC dependency parser and
archive response file generation.
Source builds need Rust, a C toolchain, `make`,
`git`, and NASM. Runtime encoding still needs the `ffmpeg` executable on
`PATH`; media inspection uses `ffprobe`.

For PyPI Trusted Publishing, register the GitHub owner `evgen2571`, repository
`vestra`, workflow `release.yml`, and environment `pypi` in the PyPI project
settings. Protect the GitHub `pypi` environment so release publication needs
maintainer approval. Publishing a GitHub release tagged `v0.1.0` then runs the
same artifact matrix and uploads its results through OIDC only after every
build and smoke test passes. The workflow verifies that the tag matches the
workspace version. Pull request and manual runs build and test artifacts
without publishing them.
