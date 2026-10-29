"""Stdlib-only native preflight and uv preparation; no engine imports."""

import ctypes
import ctypes.util
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

from scripts.benchmark import ROOT


def executable(name, system=None):
    """Respect PATH/PATHEXT, including Windows executable suffixes."""
    system = system or platform.system()
    return shutil.which(name) or (
        shutil.which(name + ".exe") if system == "Windows" else None
    )


def version(command):
    try:
        result = subprocess.run(
            command, cwd=ROOT, capture_output=True, text=True, timeout=20
        )
        if result.returncode:
            return None
        lines = (result.stdout + result.stderr).strip().splitlines()
        return lines[0] if lines else None
    except (OSError, subprocess.TimeoutExpired):
        return None


def libclang_candidates(system):
    names = {
        "Windows": ["libclang.dll"],
        "Darwin": ["libclang.dylib"],
        "Linux": ["libclang.so", "libclang.so.1"],
    }[system]
    found = ctypes.util.find_library("clang")
    if found:
        yield found
    roots = [Path(p) for p in (os.environ.get("LIBCLANG_PATH"),) if p]
    if system == "Windows":
        roots += [Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "LLVM/bin"]
    elif system == "Darwin":
        roots += [
            Path("/opt/homebrew/opt/llvm/lib"),
            Path("/usr/local/opt/llvm/lib"),
            Path("/Library/Developer/CommandLineTools/usr/lib"),
        ]
    else:
        roots += [Path("/usr/lib"), Path("/usr/lib/x86_64-linux-gnu")]
        roots += sorted(Path("/usr/lib").glob("llvm-*/lib"), reverse=True)
    for root in roots:
        if root.is_file():
            yield str(root)
        for name in names:
            if (root / name).is_file():
                yield str(root / name)


def find_libclang(system):
    candidates = list(libclang_candidates(system))
    for candidate in candidates:
        try:
            library = ctypes.CDLL(candidate)
            getattr(library, "clang_createIndex")
            return candidate
        except (OSError, AttributeError):
            pass
    # uv's portable Python can use a different libc from a Nix compiler. Test
    # bindgen's native host ABI before declaring a usable native library missing.
    for candidate in candidates:
        if native_library_check(candidate, system):
            return candidate
    return None


def native_library_check(candidate, system):
    compiler = executable(
        os.environ.get("CC", "cl" if system == "Windows" else "cc"), system
    )
    if not compiler:
        return False
    code = (
        '#include <windows.h>\nint main(int n, char **v) { HMODULE h = LoadLibraryA(v[1]); return !(h && GetProcAddress(h, "clang_createIndex")); }'
        if system == "Windows"
        else '#include <dlfcn.h>\nint main(int n, char **v) { void *h = dlopen(v[1], RTLD_NOW); return !(h && dlsym(h, "clang_createIndex")); }'
    )
    try:
        with tempfile.TemporaryDirectory(prefix="vestra-clang-") as temporary:
            source = Path(temporary) / "check.c"
            target = Path(temporary) / ("check.exe" if system == "Windows" else "check")
            source.write_text(code, encoding="utf-8")
            command = (
                [compiler, "/nologo", str(source), f"/Fe:{target}"]
                if system == "Windows"
                else [
                    compiler,
                    str(source),
                    "-o",
                    str(target),
                    *(["-ldl"] if system == "Linux" else []),
                ]
            )
            subprocess.run(
                command, cwd=temporary, check=True, capture_output=True, timeout=30
            )
            return (
                subprocess.run(
                    [str(target), candidate], capture_output=True, timeout=20
                ).returncode
                == 0
            )
    except (OSError, subprocess.SubprocessError):
        return False


def compiler_check(directory, system):
    """Compile AND link C/C++ to detect uninitialized MSVC/SDK environments."""
    for language, variable, defaults in (
        ("c", "CC", ["cl"] if system == "Windows" else ["cc", "clang", "gcc"]),
        ("cpp", "CXX", ["cl"] if system == "Windows" else ["c++", "clang++", "g++"]),
    ):
        candidates = [os.environ[variable]] if os.environ.get(variable) else defaults
        compiler = next(
            (executable(c, system) for c in candidates if executable(c, system)), None
        )
        if not compiler:
            raise ValueError(f"{language} compiler missing")
        source = directory / f"check.{language}"
        source.write_text(
            "#include <vector>\nint main() { std::vector<int> v(3); return v[0]; }\n"
            if language == "cpp"
            else "#include <stdlib.h>\nint main(void) { return 0; }\n",
            encoding="utf-8",
        )
        target = directory / (
            f"check-{language}.exe" if system == "Windows" else f"check-{language}"
        )
        command = (
            [compiler, "/nologo", str(source), f"/Fe:{target}"]
            if system == "Windows"
            else [compiler, str(source), "-o", str(target)]
        )
        result = subprocess.run(
            command, cwd=directory, capture_output=True, text=True, timeout=60
        )
        if result.returncode:
            raise ValueError(
                f"{language} compile/link failed: {result.stdout[-2000:]} {result.stderr[-2000:]}"
            )
        subprocess.run([str(target)], check=True, timeout=10, capture_output=True)


def preflight(directory, *, build=True):
    system = platform.system()
    rows = []

    def check(name, function, guidance):
        try:
            detail = function()
            if not detail:
                raise ValueError("not available")
            rows.append(dict(name=name, status="passed", detail=detail))
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            rows.append(
                dict(name=name, status="failed", error=str(error), guidance=guidance)
            )

    check(
        "platform",
        lambda: system in {"Windows", "Darwin", "Linux"}
        and (system != "Windows" or platform.machine().lower() in {"amd64", "x86_64"}),
        "Use Windows x64, macOS, or Linux.",
    )
    check(
        "Python",
        lambda: sys.version if (3, 11) <= sys.version_info[:2] <= (3, 14) else None,
        "Run uv run --no-project --python 3.13 scripts/collect-benchmarks.py",
    )
    tools = {
        "uv": ["--version"],
        "git": ["--version"],
        "ffmpeg": ["-version"],
        "ffprobe": ["-version"],
    }
    if build:
        tools.update(
            cargo=["--version"], rustc=["--version"], make=["--version"], nasm=["-v"]
        )
        if system == "Linux":
            tools["pkg-config"] = ["--version"]
        if system == "Windows":
            tools["sh"] = ["--version"]
    for name, args in tools.items():
        check(
            name,
            lambda n=name, a=args: version([executable(n) or n, *a]),
            f"Install {name} and put it on PATH; see benchmarks/COLLECTING.md.",
        )
    if build:
        check(
            "GNU Make",
            lambda: "GNU Make" in (version(["make", "--version"]) or ""),
            "Install GNU Make as make on PATH; BSD make is not sufficient.",
        )
    check(
        "libx264/FFV1",
        lambda: codec_check(),
        "Install FFmpeg with functioning libx264 and FFV1 encoders.",
    )
    check(
        "writable output",
        lambda: writable(directory),
        "Choose a writable --output directory.",
    )
    check(
        "free disk",
        lambda: shutil.disk_usage(directory).free >= (6 if build else 1) * 2**30,
        "Free at least 6 GiB for builds, plus fixture and output space.",
    )
    memory = physical_memory()
    check(
        "RAM",
        lambda: memory is None or memory >= 2 * 2**30,
        "At least 2 GiB RAM is required; 8 GiB or more is recommended for native builds and rendering.",
    )
    if build and system in {"Windows", "Darwin", "Linux"}:
        with tempfile.TemporaryDirectory(
            prefix="vestra-preflight-", dir=directory
        ) as temporary:
            check(
                "C/C++ compiler and linker",
                lambda: compiler_check(Path(temporary), system)
                or "compile/link/run OK",
                "Windows: use x64 Native Tools Command Prompt for VS 2022 with Windows SDK. macOS: xcode-select --install. Linux: install build-essential and clang.",
            )
        check(
            "libclang",
            lambda: find_libclang(system),
            "Install LLVM/libclang and set LIBCLANG_PATH to its library directory.",
        )
        if system == "Darwin":
            check(
                "Xcode tools",
                lambda: version(["xcrun", "--find", "clang"]),
                "Run xcode-select --install.",
            )
        if system == "Windows":
            check(
                "Rust MSVC x64 target",
                msvc_host,
                "Use x64 Native Tools Command Prompt for VS 2022; install Rust x86_64-pc-windows-msvc.",
            )
            check(
                "FFmpeg shell tools",
                shell_tools,
                "Put Git for Windows usr/bin and bin on PATH (sh, sed, awk, diff, tr). Keep MSVC linker ahead of Git's link.exe.",
            )
    return dict(
        status="passed" if all(r["status"] == "passed" for r in rows) else "failed",
        build_required=build,
        checks=rows,
        memory_bytes=memory,
        network="Initial uv/Cargo/FFmpeg source downloads require network access; download failures retain diagnostics.",
    )


def msvc_host():
    result = subprocess.run(
        ["rustc", "-vV"], capture_output=True, text=True, timeout=20, check=True
    )
    return "host: x86_64-pc-windows-msvc" in result.stdout


def physical_memory():
    try:
        if platform.system() == "Windows":

            class Status(ctypes.Structure):
                _fields_ = [
                    ("length", ctypes.c_ulong),
                    ("load", ctypes.c_ulong),
                    *[
                        (name, ctypes.c_ulonglong)
                        for name in (
                            "total",
                            "available",
                            "page",
                            "available_page",
                            "virtual",
                            "available_virtual",
                            "extended",
                        )
                    ],
                ]

            status = Status()
            status.length = ctypes.sizeof(status)
            return (
                status.total
                if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status))
                else None
            )
        if platform.system() == "Darwin":
            return int(
                subprocess.check_output(
                    ["sysctl", "-n", "hw.memsize"], text=True, timeout=10
                )
            )
        return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
    except (OSError, ValueError, AttributeError, subprocess.SubprocessError):
        return None


def shell_tools():
    subprocess.run(
        ["sh", "-c", "command -v sed awk diff tr >/dev/null"],
        check=True,
        capture_output=True,
        timeout=20,
    )
    return "shell utilities available"


def codec_check():
    for codec in ("libx264", "ffv1"):
        subprocess.run(
            [
                "ffmpeg",
                "-v",
                "error",
                "-nostdin",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=32x32:r=2",
                "-frames:v",
                "2",
                "-c:v",
                codec,
                "-threads",
                "1",
                "-f",
                "null",
                "-",
            ],
            capture_output=True,
            check=True,
            timeout=30,
        )
    return "two moving frames encoded by each codec"


def writable(directory):
    with tempfile.TemporaryFile(dir=directory) as stream:
        stream.write(b"vestra")
    return True


def sync_command(interpreter, *, dependencies_only=False):
    command = [
        "uv",
        "sync",
        "--locked",
        "--python",
        str(interpreter),
        "--extra",
        "benchmark",
    ]
    if dependencies_only:
        return [*command, "--no-install-project"]
    return [
        *command,
        # Editable wheels point at a checkout-level extension that another
        # development build can replace. Rebuild an isolated wheel from the
        # current sources, retaining Cargo's own incremental build cache.
        "--no-editable",
        "--no-cache",
        "--no-build-isolation-package",
        "vestra",
        "--reinstall-package",
        "vestra",
        "--config-settings",
        "build-args=--profile release --locked",
    ]


def managed_environment():
    env = os.environ.copy()
    env.pop("VIRTUAL_ENV", None)
    env["UV_PROJECT_ENVIRONMENT"] = str(ROOT / "target/benchmark-env")
    return env
