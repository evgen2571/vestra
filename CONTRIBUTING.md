# Contributing to Vestra

Contributions can include bug fixes, documentation improvements, tests and new
features. This guide walks through reporting a problem, setting up a checkout,
checking a change and opening a pull request.

## 1. Choose a change

Check the [issue tracker](https://github.com/evgen2571/vestra/issues) for existing
reports and related pull requests before starting. For a larger feature or a
public API change, open an issue to discuss the approach first.

For a bug report, include:

- A small Python script or JSON project that reproduces the problem.
- What you expected and what actually happened.
- Your Vestra version, operating system and relevant error messages.
- The rendering backend and adapter when reporting a GPU problem.

For example, a report about a missing video frame should include the smallest
composition that shows it, the render command and the affected timestamp.
Remove personal paths and sensitive information from logs before sharing them.

## 2. Set up a checkout

Fork the repository on GitHub, clone your fork and create a branch for the change:

```bash
git clone https://github.com/YOUR_USERNAME/vestra.git
cd vestra
git switch -c fix-description
```

The repository provides a pinned development environment through Nix. From the
repository root, enter it and build the editable Python package:

```bash
nix develop
just python-sync
```

Run `just` to list available commands. If you do not use Nix, follow the native
setup instructions in [installation](docs/getting-started/installation.md#build-from-source--development)
and [testing](docs/development/testing.md). Rendering requires `ffmpeg` and
`ffprobe` on `PATH`.

## 3. Make and test your change

Keep each pull request focused on one problem. Follow the surrounding code style
and update the user documentation when behavior changes. A bug fix should include
a test that reproduces the failure and passes with the fix.

Run the tests closest to the change while working. For example, to check the
public documentation links:

```bash
just docs-check
```

For Rust changes, run tests for the affected crate:

```bash
cargo test -p vestra-core
```

For Python changes, run the Python test suite:

```bash
just python-test
```

Vestra's public Rust/Python APIs, project schema, CLI and diagnostics are shared
contracts. Changes to these may also need updated type stubs, schema checks or
reference documentation. See [testing](docs/development/testing.md) for the
checks relevant to your change, and the [architecture overview](docs/development/architecture/overview.md)
for an introduction to the codebase.

Rendering changes need checks of the actual frames or media output as well as
unit tests. For changes affecting both CPU and WGPU, verify both backends.
Follow [GPU validation](docs/development/gpu-validation.md) when testing on
hardware, and state whether your results used a hardware or software adapter.

## 4. Check the finished change

Before opening a pull request, format your changes and run the contributor checks:

```bash
just fmt
just check
```

Run any additional checks needed for your change, such as `just python-test` for
Python changes or `just docs-check` for documentation changes. Review your diff
for unrelated edits and generated files that do not belong in the pull request.
The [testing guide](docs/development/testing.md) describes the local and CI checks
in detail.

## 5. Open a pull request

Push your branch to your fork and open a pull request against `main`. Include a
short description of the problem, what changed, any related issue and the checks
you ran. For example:

```text
Fix a missing frame at the end of a video layer.

The final frame now remains visible until the layer ends.
Adds a regression test for the layer's end timestamp.

Validation: focused Rust tests and just check.
```

If a relevant check could not run, explain why. Maintainers may ask for changes
or additional tests during review; add follow-up commits to the same branch.

## Licensing

Vestra is licensed under [MIT](LICENSE). Third-party example assets retain their
own licenses; preserve their attribution and check the license before adding
new media.
