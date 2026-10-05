# Releasing the Python package

The workspace version in `Cargo.toml` is the release version for every Rust
crate. Maturin reads the Python package version from `vestra-python` through
`pyproject.toml`'s dynamic version setting. Project JSON and RenderEvent both
use schema version 1 for the first public `0.1.0` release.

Run `just check` and `just python-test` before building release artifacts.
The artifact matrix runs on published releases or can be started manually;
it does not run on pull requests. The workflow builds and smoke
tests CPython 3.11 through 3.14 wheels on Linux x86_64 and aarch64, macOS Intel
and Apple Silicon, and Windows x64. The workflow also builds a source
distribution and installs it in a clean environment. Each install must pass
`scripts/verify-wheel.py`, which checks the installed version, creates and
validates a project, renders an MP4, and checks its AAC audio. Review every
matrix job and its uploaded artifact before publishing. Each wheel job also
archives the actual corresponding FFmpeg source and modifications.

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
maintainer approval. Publishing a GitHub release tagged `v0.1.1` then runs the
same artifact matrix and uploads its results through OIDC only after every
build and smoke test passes. The workflow verifies that the tag matches the
workspace version. Manual runs build and test artifacts
without publishing them.

## Maintainer release checklist

Before creating a tag or publishing a release:

1. Run `just style check python-test examples showcase-smoke` and `just wheel-smoke`.
2. Validate and visually inspect the public showcase outputs using the examples
   instructions. Check documentation links with `just docs-check`.
3. Push the reviewed release-preparation changes, then manually run
   `.github/workflows/release.yml` on that exact revision. All 20 wheel jobs and
   the source-distribution job must pass at least once before release.
4. Download and inspect the resulting wheel/sdist artifacts. The manual event
   cannot enter either the PyPI publish job or the GitHub attachment job.
5. Configure PyPI Trusted Publishing and the protected `pypi` environment.
6. Review the release notes and the history/privacy decision before making the
   repository public. Creating the tag and publishing remain maintainer actions.

Version/license inheritance and release-tag consistency are checked before
artifact builds. The sdist smoke environment installs Maturin 1.14.1 explicitly
and builds without isolation so this verification uses the same build backend
as the wheel jobs. Windows checks native command exit codes at every install
and smoke boundary.

On a published release, tested `*.whl` and `*.tar.gz` distributions are also
attached to the GitHub release, together with the corresponding FFmpeg source
archives. PyPI publication waits for successful attachment so these source
downloads are available before wheels are published. Only that attachment job has `contents: write`;
only PyPI publication has `id-token: write` and the protected environment.
Attachment retries replace files with the same name. The publication job still
requires every artifact build and smoke test to succeed.

## Bundled-library source and licenses

The MIT license covers Vestra software; wheels also contain statically linked
LGPL FFmpeg libraries. The [third-party notices](../../THIRD_PARTY_NOTICES.md)
and LGPL text are included in wheel/sdist metadata. Each wheel job runs
`scripts/archive-ffmpeg-source.py` against the exact build checkout. Its ZIP
contains tracked source files with applied changes, a diff, upstream revision,
portable configure options and the vendored build recipe. Compiler paths,
installation prefixes and generated build objects are excluded.

Before publication, inspect each archive and confirm that the source matches
its wheel. Keep the source archives on the release download page alongside
the wheels and source distribution. Review FFmpeg redistribution requirements
against [upstream licensing guidance](https://ffmpeg.org/legal.html); release
checks provide source material and notices, not a legal compliance certification.

The relevant upstream contracts are [Maturin Action](https://github.com/PyO3/maturin-action),
[PyPI Trusted Publishing](https://docs.pypi.org/trusted-publishers/using-a-publisher/)
and [GitHub release uploads](https://cli.github.com/manual/gh_release_upload).
