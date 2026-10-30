"""Human summaries and allowlisted, sanitized shareable archives."""

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
from statistics import median
import uuid
import zipfile

from scripts.benchmark import ROOT

VARIABLES = {
    "VESTRA_WGPU_BACKEND",
    "VESTRA_WGPU_FORCE_FALLBACK",
    "VESTRA_WGPU_IN_FLIGHT",
    "RAYON_NUM_THREADS",
    "RUSTFLAGS",
}


def new_directory(parent, system):
    parent.mkdir(parents=True, exist_ok=True)
    for _ in range(10):
        name = f"vestra-bench-{system.lower()}-{datetime.now(timezone.utc):%Y%m%dT%H%M%SZ}-{uuid.uuid4().hex[:8]}"
        directory = parent / name
        try:
            directory.mkdir()
            return directory
        except FileExistsError:
            continue
    raise FileExistsError("could not allocate a unique results directory")


def sanitize(value, *, private_roots=()):
    if isinstance(value, dict):
        return (
            {
                k: sanitize(v, private_roots=private_roots)
                for k, v in value.items()
                if not re.search(r"(?i)token|password|secret|api.?key|authorization", k)
                and k not in {"tags", "environment_variables"}
                and (k != "variables" or isinstance(v, dict))
            }
            if "variables" not in value
            else sanitize(
                {
                    **{k: v for k, v in value.items() if k != "variables"},
                    "execution_controls": {
                        k: v for k, v in value["variables"].items() if k in VARIABLES
                    },
                },
                private_roots=private_roots,
            )
        )
    if isinstance(value, (list, tuple)):
        return [sanitize(v, private_roots=private_roots) for v in value]
    if not isinstance(value, str):
        return value
    replacements = [(str(Path.home()), "<home>"), (str(ROOT), "<checkout>")]
    replacements += [(str(p), f"<private-{i}>") for i, p in enumerate(private_roots)]
    for source, replacement in sorted(replacements, key=lambda p: -len(p[0])):
        if len(source) > 3:
            value = value.replace(source, replacement).replace(
                source.replace("\\", "/"), replacement
            )
    value = re.sub(r"(?i)[A-Z]:[\\/][^\s\"'<>]*", "<absolute-path>", value)
    value = re.sub(
        r"/(?:home|Users|tmp|private|run/user)/[^\s\"'<>]*", "<absolute-path>", value
    )
    value = re.sub(
        r"(?i)(?:[\w-]*(?:token|secret|password|api[_-]?key)|authorization)\s*[:=]\s*[^\s,;]+",
        "<credential-redacted>",
        value,
    )
    value = re.sub(r"https?://[^/\s:@]+:[^/\s@]+@", "https://<credentials>@", value)
    username = os.environ.get("USERNAME") or os.environ.get("USER")
    if username and len(username) >= 3:
        value = re.sub(rf"\b{re.escape(username)}\b", "<user>", value)
    return value


def archive(directory, *, private_roots=()):
    """Exclude all media/images/binaries and local scratch; never follow links."""
    target = directory.with_suffix(".zip")
    with zipfile.ZipFile(target, "x", compression=zipfile.ZIP_DEFLATED) as bundle:
        for path in sorted(directory.rglob("*")):
            relative = path.relative_to(directory)
            if (
                path.is_symlink()
                or not path.is_file()
                or "local" in relative.parts
                or path.suffix not in {".json", ".jsonl", ".md", ".log"}
            ):
                continue
            data = path.read_text(encoding="utf-8", errors="replace")
            if path.suffix == ".json":
                try:
                    data = (
                        json.dumps(
                            sanitize(json.loads(data), private_roots=private_roots),
                            indent=2,
                            allow_nan=False,
                        )
                        + "\n"
                    )
                except ValueError:
                    data = (
                        json.dumps(
                            dict(
                                status="incomplete",
                                error="worker left invalid JSON",
                                sanitized_partial=sanitize(
                                    data, private_roots=private_roots
                                ),
                            ),
                            indent=2,
                        )
                        + "\n"
                    )
            elif path.suffix == ".jsonl":
                data = "".join(
                    json.dumps(
                        sanitize(row, private_roots=private_roots),
                        allow_nan=False,
                    )
                    + "\n"
                    for row in jsonl_rows(data)
                )
            else:
                if path.suffix == ".log" and len(data) > 4 * 2**20:
                    data = "[log tail; truncated to 4 MiB]\n" + data[-4 * 2**20 :]
                data = sanitize(data, private_roots=private_roots)
            bundle.writestr(f"{directory.name}/{relative.as_posix()}", data)
    return target


def number(value, divisor=1):
    return "unavailable" if value is None else f"{value / divisor:.3f}"


def documents(paths):
    for path in sorted(paths):
        try:
            yield path, json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue  # A killed worker may leave incomplete evidence; task status records failure.


def jsonl_rows(data):
    """Keep valid samples and explicit evidence of a worker's truncated write."""
    for line in data.splitlines():
        if not line.strip():
            continue
        try:
            row = json.loads(line)
            # Reject non-finite values before archive serialization too.
            json.dumps(row, allow_nan=False)
            yield row
        except ValueError:
            yield dict(
                status="incomplete",
                error="worker left invalid JSONL sample",
                sanitized_partial=line,
            )


def summarize(directory, manifest, system):
    lines = [
        "# Vestra benchmark collection",
        "",
        f"Profile: {manifest['profile']}; smoke: {manifest['smoke']}. Overall: **{manifest['status']}**.",
        "",
        f"Hardware: {system.get('cpu', 'unavailable')}; {system.get('machine')}; {system.get('platform')}; logical CPUs {system.get('logical_cpus')}; RAM {number(system.get('memory_bytes'), 2**30)} GiB.",
        "",
        "Task | Execution | Validation | Detail",
        "--- | --- | --- | ---",
    ]
    for row in manifest["tasks"]:
        detail = (
            str(row.get("error", row.get("reason", "")))
            .replace("\n", " ")
            .replace("|", "/")[:600]
        )
        lines.append(
            f"{row['name']} | {row['status']} | {row.get('validation', 'unavailable')} | {detail}"
        )
    lines += [
        "",
        "## Vestra samples",
        "",
        "Suite/backend/scenario | Median wall s | Range s | Effective FPS",
        "--- | ---: | ---: | ---:",
    ]
    for path, data in documents(directory.glob("vestra/**/suite.json")):
        for row in data["records"]:
            samples = row["samples"]
            walls = [s["wall_ms"] for s in samples]
            fps = median(
                s["result"]["total_frames"] / (s["wall_ms"] / 1000) for s in samples
            )
            lines.append(
                f"{data['definition']['name']}/{data['definition']['backend']}/{row['scenario']} | {median(walls) / 1000:.3f} | {min(walls) / 1000:.3f}–{max(walls) / 1000:.3f} | {fps:.2f}"
            )
    lines += [
        "",
        "## MoviePy comparison",
        "",
        "Scenario/engine | Median wall s | Range s | FPS | Tree RSS MiB",
        "--- | ---: | ---: | ---: | ---:",
    ]
    timings = {}
    for path, data in documents(directory.glob("moviepy/**/comparison.json")):
        for row in data["records"]:
            if not row.get("summary"):
                continue
            s = row["summary"]
            timings[(row["scenario"], row["engine"])] = s["wall_ms"]["median"]
            wall = s["wall_ms"]
            rss = s["peak_tree_rss_bytes"]
            lines.append(
                f"{row['scenario']}/{row['engine']} | {wall['median'] / 1000:.3f} | {wall['min'] / 1000:.3f}–{wall['max'] / 1000:.3f} | {s['effective_fps']['median']:.2f} | {number(rss['median'] if rss else None, 2**20)}"
            )
    for (scenario, engine), wall in sorted(timings.items()):
        reference = timings.get((scenario, "moviepy-cpu"))
        if engine != "moviepy-cpu" and reference:
            lines += [
                "",
                f"{scenario}/{engine}: {wall / reference:.3f}× MoviePy worker wall time on this host. Read validation status above before interpreting this ratio.",
            ]
    for _, data in documents([directory / "cache/results.json"]):
        lines += [
            "",
            "## Cache access",
            "",
            "Pattern | Cache-off ms | Cache-on ms | On/off ratio | MoviePy get_frame ms | Exact pixels",
            "--- | ---: | ---: | ---: | ---: | ---",
        ]
        for row in data["patterns"]:
            if row.get("summary"):
                s = row["summary"]
                lines.append(
                    f"{row['pattern']} | {s['off_us'] / 1000:.3f} | {s['on_us'] / 1000:.3f} | {s['on_us'] / s['off_us']:.3f} | {s['moviepy_us'] / 1000:.3f} | {row['correctness']}"
                )
        lines += [
            "",
            "Ratios describe this request sequence only. Sequential access often benefits from cursor retention without LRU hits. MoviePy public get_frame timings and RSS are recorded separately; it exposes no equivalent native cache counters.",
        ]
    lines += ["", "## Sustained rendering", ""]
    for path, result in documents(directory.glob("sustained/*/result.json")):
        wall_ms = result.get("measurement", {}).get("wall_ms")
        fps = result.get("frames", 0) / (wall_ms / 1000) if wall_ms else None
        lines.append(
            f"- {path.parent.name}: {result.get('status')}; requested {result.get('duration')} s; {result.get('frames')} frames; worker wall {number(wall_ms, 1000)} s; average {number(fps)} FPS; peak tree RSS {number(result.get('measurement', {}).get('peak_tree_rss_bytes'), 2**20)} MiB."
        )
        series = path.parent / "resources.jsonl"
        if series.is_file():
            rows = [
                row
                for row in jsonl_rows(series.read_text())
                if isinstance(row, dict) and "tree_rss_bytes" in row
            ]
            thirds = [[], [], []]
            if wall_ms:
                for row in rows:
                    elapsed = row.get("elapsed_seconds")
                    if elapsed is not None and elapsed >= 0:
                        phase = min(2, int(elapsed / (wall_ms / 1000) * 3))
                        thirds[phase].append(row)
            phases = []
            for phase in thirds:
                values = [
                    r["tree_rss_bytes"]
                    for r in phase
                    if r["tree_rss_bytes"] is not None
                ]
                phases.append(number(median(values) if values else None, 2**20))
            lines.append(
                f"  Early/middle/late median tree RSS MiB: {' / '.join(phases)}. This alone does not diagnose a leak."
            )
    lines += [
        "",
        "## Methodology and limits",
        "",
        "Builds, fixture preparation, validation and report generation are outside render timings. Vestra canonical wall samples time each one-shot render; MoviePy comparison wall includes full worker startup/imports. Decoder access times exclude opens and correctness scans. Internal engine stages overlap and are not interchangeable with MoviePy write_videofile timings.",
        "",
        "Process-tree RSS is a sampled lower bound; shared pages may be counted more than once. CPU is a sampled observed-tree estimate (one core = 100%); short-lived descendants may be missed. Missing metrics are null, never zero; RSS is not GPU VRAM. Sustained series retain early/steady/late observations; decoder/cache counters are final totals where live counters are unavailable.",
        "",
        "Only confirmed integrated/discrete adapters qualify for hardware results; no CPU/software fallback is accepted. WGPU uses CPU decode and libx264 encoding. Resampling/rounding/mask edges differ between engines; visual checks do not claim exact pixel equivalence. Smoke proves execution, not comparative performance or multi-hour stability.",
        "",
        "Use the established comparator only for compatible captures on the same machine. Cross-machine results describe hosts and cannot establish a code regression. Sources and OS page caches can be warm. Inspect distributions and repeat small differences.",
        "",
        "The ZIP excludes media, images and local scratch and sanitizes private paths and credentials. Original logs/media may remain locally; do not share the whole result folder. No telemetry or upload occurs.",
        "",
    ]
    (directory / "summary.md").write_text(sanitize("\n".join(lines)), encoding="utf-8")
