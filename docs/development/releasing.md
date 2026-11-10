# Releasing Vestra

The release workflow prepares and tests artifacts from one exact commit before
publication. A manual candidate run publishes nothing. A version tag runs the
same checks, stages a GitHub prerelease with corresponding source, then waits
for protected PyPI approval before marking the release complete.

## Maintainer release checklist

1. Update the workspace version and release notes. Run
   `just style check python-test examples showcase-smoke release-check` and
   `just wheel-smoke`; visually inspect showcase outputs as described in
   [testing](testing.md). Review the notices and redistribution terms below.
2. Push the reviewed revision. For a candidate, use Actions → **Release
   artifacts** → **Run workflow**, select the reviewed workflow branch, set
   `ref` to that commit SHA and leave `publish` false. The ref is resolved once;
   all artifact jobs check out the resulting SHA. Manual runs may also target
   a version tag. Wait for every build, smoke and inventory job.
3. Download all `release-*` and `sdist` workflow artifacts into one directory.
   Inspect `release-manifest.json` and run `sha256sum -c SHA256SUMS` (or the
   equivalent hash tool). Inspect the source ZIPs and installation notes;
   candidate artifacts are retained by Actions' configured retention policy.
4. Configure PyPI Trusted Publishing for owner `evgen2571`, repository
   `vestra`, workflow `release.yml`, environment `pypi`. Protect the GitHub
   `pypi` environment with required maintainer review and appropriate tag rules.
   Publication fails before building if the environment has no required
   reviewers. Confirm approval settings before pushing a release tag.
5. Push the immutable `v<version>` tag. The workflow rebuilds and validates
   from that tag; a prior branch candidate is evidence, not an artifact promoted
   to a different revision. Alternatively, dispatch the workflow **from the
   same version tag**, with `ref` set to that tag and `publish` true:

   ```bash
   gh workflow run release.yml --ref vVERSION -f ref=vVERSION -f publish=true
   ```

   Replace `vVERSION` with the workspace version tag. Selecting a workflow
   branch while supplying a tag input is rejected for publication.
6. Review the staged GitHub prerelease, manifest and source downloads, then
   approve the `pypi` environment. PyPI uses OIDC; no long-lived upload token
   is configured. Only staging/promotion jobs have `contents: write`, and only
   the publication job has `id-token: write`.
7. Confirm PyPI distributions and the final GitHub release. The workflow
   promotes the prerelease only after PyPI succeeds. Edit generated release
   notes as needed. Do not create a complete public release before validation.

## Version and artifact contract

The workspace version in `Cargo.toml` is the version for every Rust crate and
the Python package. Maturin reads it from `vestra-python`; `pyproject.toml`
keeps its dynamic version setting. Publication requires `v<workspace-version>`
and verifies that the tag still identifies the tested commit before staging,
PyPI publication and final promotion. Never move or reuse a published tag.

`.github/workflows/artifacts.yml` is shared by release preparation and
packaging-related PR checks. It builds:

- Five `cp311-abi3` wheels: Linux x86_64/aarch64, macOS Intel/Apple Silicon,
  and Windows x64. Each wheel is installed in fresh CPython 3.11,
  3.12, 3.13 and 3.14 environments, for 20 installed-wheel smoke jobs.
- Five separate native CLI archives, named
  `ve-<version>-<rust-target>.tar.gz` (Unix) or `.zip` (Windows), with SHA-256
  sidecars, MIT license, third-party notices and dependency license texts.
  CLI compilation selects `vestra-cli`, independently of Python bindings.
- One Python sdist, installed and tested in a fresh environment.
- Ten corresponding FFmpeg source ZIPs, one for each wheel/CLI build.
  Separate Cargo target directories make source association explicit. No
  equivalence between platform builds or wheel/CLI builds is assumed.
- `release-manifest.json` with the exact commit and all artifact hashes,
  plus `SHA256SUMS`. Inventory validation requires every target and verifies
  each source ZIP's embedded binary hash and Vestra revision.

Wheels use CPython's stable ABI: one wheel per platform supports CPython
3.11–3.14. PyPy and free-threaded CPython are not supported.
`benchmarks/python-abi.py` measures Python/native conversion overhead
separately from render throughput.

## Artifact verification

Installed-wheel checks cover metadata, typing files, native APIs, serialized
projects, frame bytes, packaged fonts with empty discovery, native media loading,
master analysis and both encoding paths. They verify H.264/AAC output, AAC
decoding and tone timing, an executable path containing spaces, and executable
selection retained by prepared sessions.

These Python checks run before CLI runtime tools are installed, in a child
process with an empty PATH, no FFmpeg override and no development library search
paths. They do not use FFprobe. Codec failures fail the artifact gate.

CLI checks verify the archive checksum, extract it outside the checkout, run
`version` and `validate`, render with CPU, and probe frames and presentation
timestamps. Fresh runners install runtime FFmpeg without the Vestra SDK for
this check. Artifact checks establish CPU packaging correctness; GPU support
requires separate adapter evidence.

## Failure and retry behavior

Build, smoke or inventory failures prevent staging and publication. A failed or
unapproved PyPI job leaves a clearly marked prerelease with source downloads,
not a release presented as complete. This also keeps corresponding FFmpeg
source publicly available before the first PyPI wheel is distributed.

Prefer **Re-run failed jobs** on the original run so retries use the same
validated artifacts. Publication compares already-uploaded PyPI filenames and
hashes before skipping existing files, allowing a partial upload to resume
while rejecting different builds under an existing filename. Before replacing
any staged file, staging checks existing GitHub asset hashes and PyPI hashes;
a rebuild cannot replace the corresponding source of already distributed binaries.
A complete GitHub release cannot be overwritten by staging.

If only promotion failed, rerun that job; do not start a new build or replace
PyPI files. Release runs are serialized to avoid concurrent uploads. GitHub and
PyPI publication is not atomic: failed promotion can leave PyPI complete and
the GitHub prerelease pending until the job is retried.

## Native compatibility and dependencies

See [CLI installation](../getting-started/installation.md#install-the-native-cli)
for executable permissions, PATH and runtime requirements. Linux CLI builds use
the same manylinux 2.28 toolchain as the wheels. macOS builds target 11.0+;
Windows builds target x64 MSVC. Smoke tests run on Ubuntu 24.04, macOS 15 and
Windows Server 2022. This is not exhaustive testing of older operating systems
or GPUs. Native Windows/macOS PR smoke checks run for non-documentation changes;
full artifact checks run for release/packaging changes and manual CI runs.

Wheels and CLI executables statically link bundled LGPL FFmpeg libraries for
media probing and video decoding. Encoding and audio processing use a separate
executable:

<span id="python-executable-dependency"></span>

| Caller | Executable selection |
| --- | --- |
| Python | Installed through `imageio-ffmpeg>=0.6,<0.7`, with `IMAGEIO_FFMPEG_EXE` for an override. |
| Rust / CLI | `ffmpeg` on PATH by default. |
| Independent output verification | Optional `ffprobe`; Vestra does not invoke it for runtime probing. |

The dependency's executable is downloaded separately from the Vestra wheel;
normal dependency resolution installs one copy. Smoke logs record executable
sizes, `-version` and `-L` output for each target. See
[NixOS troubleshooting](../troubleshooting/ffmpeg.md#nixos) for native runtime
requirements; an executable running successfully does not prove that a manylinux
extension can load on the same host.

Source builds need Rust, a C toolchain, libclang, `make`, Git and NASM. The
vendored `ffmpeg-sys-next` recipe removes `-march=native` and fixes MSVC dependency
parsing and archive response files.

## Bundled-library source and licenses

MIT covers Vestra software. The
[third-party notices](https://github.com/evgen2571/vestra/blob/develop/THIRD_PARTY_NOTICES.md)
and license texts accompany redistributed wheel/CLI binaries. Each job runs
`scripts/archive-ffmpeg-source.py --search-root target/wheel` or `target/cli`,
with `--artifact` identifying the corresponding packaged binary. Discovery
requires exactly one configured source tree; missing or ambiguous builds fail
instead of choosing an arbitrary checkout.

Each source ZIP contains the actual tracked FFmpeg source with applied edits,
its diff, upstream revision, portable configure/compiler options, license texts,
notices and vendored build recipe. Host paths and generated objects are omitted.
`BUILD.json` records the Vestra commit and artifact name/hash. Do not deduplicate
archives merely because they fetched the same upstream revision: configuration,
patches, target flags and binary association also matter. The build currently
fetches FFmpeg's `release/8.1` branch; the archived exact revision is authoritative,
and later rebuilds are not promised to be bit-for-bit identical.

**Static-linking review remains a release responsibility.** LGPL 2.1 §6 includes
requirements for modification/relinking and reverse engineering for debugging
such modifications. §6(a) permits complete work material as object code and/or
source code, but source ZIPs alone do not demonstrate that recipients can relink
the Rust binaries with modified FFmpeg. Before distribution, review the actual
terms and verify a usable rebuild/relink route for each target, supplying any
additional object material or instructions that route needs.

The source ZIP README describes rebuilding its FFmpeg tree and adapting the
vendored fetch step to use modified source, followed by a clean Maturin or CLI
Cargo rebuild. When reusing the already patched Windows tree, skip the two
`patch_windows_*` calls instead of applying the patches a second time. Retain
the matching complete Vestra source from the GitHub tag (including the
CLI crate), Cargo.lock, patches and build instructions alongside the corresponding
source. The Python sdist contains the extension and its dependencies, not the
CLI crate; use it only for Python-module rebuilds. This route
has not been validated as an LGPL-compliant relinking kit on all platforms;
maintainer/legal review is still required. Keep source downloads accessible for
as long as required by the chosen redistribution route.

### Python executable licenses

The `imageio-ffmpeg` Python wrapper is BSD-2-Clause. Its FFmpeg executables have
separate licenses and enabled components from Vestra's linked LGPL libraries.
For version 0.6.0:

| Executable | Reported license / restriction |
| --- | --- |
| Linux and Windows | GPLv3-or-later; includes GPL codecs such as libx264. |
| macOS ARM64 | GPLv2-or-later. |
| macOS Intel | Built with `--enable-nonfree`; explicitly reports that it is not legally redistributable. |

Do not mirror or bundle the macOS Intel executable with an application. Arrange
a redistributable executable and dependency package, then select that build
with `IMAGEIO_FFMPEG_EXE`. The override alone does not remove the installed
dependency binary. Automatic installation downloads from the dependency's
publisher; Vestra's wheel does not contain these executables.

When updating or repackaging the dependency, inspect each platform's `-version`,
`-L` and wheel notices, and review source and redistribution obligations. Keep
the linked-library obligations above separate. The
[third-party notices](https://github.com/evgen2571/vestra/blob/develop/THIRD_PARTY_NOTICES.md)
and [imageio binary builds](https://github.com/imageio/imageio-binaries) provide
the dependency records.

Upstream references: [FFmpeg licensing](https://ffmpeg.org/legal.html),
[LGPL 2.1 §6](https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html#section6),
[Maturin stable ABI](https://pyo3.rs/v0.29.0/building-and-distribution#py_limited_apiabi3),
[PyPI Trusted Publishing](https://docs.pypi.org/trusted-publishers/using-a-publisher/)
and [GitHub release uploads](https://cli.github.com/manual/gh_release_upload).
