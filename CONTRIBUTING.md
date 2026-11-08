# Contributing to Vestra

Start with a focused bug fix, documentation improvement or test. Check the
[issue tracker](https://github.com/evgen2571/vestra/issues) for related work;
discuss larger features and public API changes in an issue before implementing them.

<span id="1-choose-a-change"></span>

## 1. Describe the problem

A useful bug report includes:

- A small Python script or JSON project that reproduces the problem.
- What you expected, what happened, and the relevant error messages.
- Your Vestra version and operating system.
- For rendering problems, the render command, affected timestamp, actual backend
  and adapter information when WGPU is involved.

For example, a missing-frame report should include the smallest composition
that shows the missing frame and its timestamp. Remove personal paths and
sensitive information from logs before sharing them.

## 2. Set up a checkout

Fork the repository on GitHub, clone your fork and branch from upstream `develop`:

```bash
git clone https://github.com/YOUR_USERNAME/vestra.git
cd vestra
git remote add upstream https://github.com/evgen2571/vestra.git
git fetch upstream develop
git switch -c fix-description upstream/develop
```

The default branch is `main`, but contributions start from the latest `develop`
and pull requests target **`develop`**.

Choose the environment for your change:

- **Documentation or site only:** follow [Documentation authoring](docs/development/documentation.md).
  `just docs-serve` previews the site without compiling the native engine.
- **Native engine or Python package:** enter the pinned environment and build
  the editable package:

  ```bash
  nix develop
  just python-sync
  ```

  Without Nix, follow [native installation](docs/getting-started/installation.md#build-from-source--development).
  Encoding and audio processing also need `ffmpeg` on `PATH`. Some optional
  media verification workflows use `ffprobe` as an independent reference.

Run `just` to list commands. Coding agents should read [AGENTS.md](AGENTS.md);
substantial changes follow the [plan convention](PLANS.md).

## 3. Make and test your change

Choose a route in the [contributor task map](docs/development/README.md#choose-a-task).
Keep the pull request focused, follow the surrounding style, and update user
documentation when behavior changes. Bug fixes need a regression test that
reproduces the failure.

Use the [check chooser](docs/development/testing.md#choose-checks-for-your-change)
to find focused tests and the checks needed before submission. Rust/Python APIs,
project schema, CLI and diagnostics are shared contracts; changes may need type
stubs, schema checks and reference updates as well as implementation tests.
Rendering changes also need [output inspection and adapter evidence](docs/development/gpu-validation.md#understand-the-test-evidence).

<span id="4-check-the-finished-change"></span>

## 4. Check and submit

Before opening a pull request:

- [ ] Run focused regression tests and the checks for each affected surface.
- [ ] For native code, run `just fmt` and `just check`, plus the additional
  checks selected in the [testing guide](docs/development/testing.md).
- [ ] For documentation-only work, run `just docs-check` and `just site-check`,
  then inspect affected pages in the browser.
- [ ] Review the diff for unrelated edits and generated files that do not belong.
- [ ] Update relevant documentation and preserve third-party asset attribution.

<span id="5-open-a-pull-request"></span>

Push your branch to your fork and open a pull request against **`develop`**.
Describe the problem and resulting behavior, link the related issue, and list
checks actually run. State why any relevant check could not run. For example:

```text
Fix a missing frame at the end of a video layer.

The final frame now remains visible until the layer ends.
Adds a regression test for the layer's end timestamp.

Validation: focused Rust tests and just check.
```

Add review follow-ups to the same branch.

## Licensing

Vestra is licensed under [MIT](LICENSE). Third-party example assets retain their
own licenses; preserve their attribution and check the license before adding media.
