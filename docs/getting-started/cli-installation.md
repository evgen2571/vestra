---
title: Install the CLI
description: Install the standalone ve command from a release archive.
---

# Install the CLI

Install `ve` to render JSON projects from a terminal. It runs independently of
Python. You need the archive for your platform and an `ffmpeg` executable on
`PATH`; use your platform's package manager to install FFmpeg before rendering.

## Download and install


Download the archive matching your machine from
[GitHub Releases](https://github.com/evgen2571/vestra/releases). The CLI runs
without Python; installing the Python wheel does not install `ve`.

| Platform | Archive target | Compatibility |
| --- | --- | --- |
| Linux Intel/AMD 64-bit | `x86_64-unknown-linux-gnu.tar.gz` | glibc 2.28+, not musl/Alpine |
| Linux ARM64 | `aarch64-unknown-linux-gnu.tar.gz` | glibc 2.28+ |
| macOS Intel | `x86_64-apple-darwin.tar.gz` | macOS 11.0+ |
| macOS Apple Silicon | `aarch64-apple-darwin.tar.gz` | macOS 11.0+ |
| Windows x64 | `x86_64-pc-windows-msvc.zip` | Windows 10+/Server 2022 tested |

Each filename starts with `ve-<version>-`. Download its `.sha256` sidecar too.
On Linux, verify and install (substitute the downloaded version):

```bash
sha256sum -c ve-<version>-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf ve-<version>-x86_64-unknown-linux-gnu.tar.gz
mkdir -p "$HOME/.local/bin"
chmod +x ve-<version>-x86_64-unknown-linux-gnu/ve
cp ve-<version>-x86_64-unknown-linux-gnu/ve "$HOME/.local/bin/ve"
ve version
```

Ensure `$HOME/.local/bin` is on PATH. Keep the extracted license files and
README. On macOS, compare `shasum -a 256 <archive>` with the sidecar, extract,
and install `ve` similarly. Binaries are not signed/notarized; follow your
organization's macOS security policy when using downloaded executables.

On Windows, compare `Get-FileHash <archive> -Algorithm SHA256` with the
sidecar, use `Expand-Archive <archive>`, and add the extracted folder containing
`ve.exe` to your user PATH. Open a new terminal and run `ve version`. Windows
may require the Microsoft Visual C++ 2015–2022 x64 Redistributable.

The native FFmpeg libraries are statically linked, but the `ffmpeg`
**executable remains an external requirement** for encoding and audio processing.
Linux also needs its standard C/C++ runtime libraries; macOS needs its system
frameworks. A GPU and compatible driver are needed only for WGPU rendering;
use `--render-backend cpu` for a first render. Release smoke coverage is Ubuntu
24.04, macOS 15 and Windows Server 2022; older compatible systems are not
exhaustively tested. Continue with the [CLI quickstart](cli-quickstart.md).


## Build instead

For platforms without an archive, use the contributor
[CLI source build](../development/source-builds.md#build-the-cli).
