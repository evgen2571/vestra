"""Portable process-tree measurements shared by benchmark collectors."""

import json
import math
import os
from pathlib import Path
import signal
import subprocess
import time


def terminate_tree(process):
    """Kill only this worker's owned tree and reap its root on every platform."""
    children = []
    try:
        import psutil

        # Nested benchmark workers may own separate Unix sessions. Include them
        # explicitly instead of assuming one process group contains the tree.
        children = psutil.Process(process.pid).children(recursive=True)
        for child in reversed(children):
            try:
                child.kill()
            except psutil.NoSuchProcess:
                pass
    except ImportError:
        pass
    except psutil.Error:
        pass
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    else:
        # taskkill handles descendants even when psutil is unavailable at bootstrap.
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if process.poll() is None:
            process.kill()
    process.wait()
    if children:
        psutil.wait_procs(children, timeout=3)


def measure_process(
    command,
    log_path,
    *,
    timeout,
    env=None,
    interval=0.02,
    series_path=None,
    progress_path=None,
    media_directory=None,
    minimum_free_bytes=None,
):
    """Measure worker wall time; stream bounded-cadence tree samples to JSONL.

    Summed RSS double-counts shared pages and excludes GPU VRAM. CPU time is a
    sampled cumulative tree estimate (including observed children that exited),
    with one core = 100%. Short-lived unobserved children can be missed.
    """
    if any(not math.isfinite(value) or value <= 0 for value in (timeout, interval)):
        raise ValueError("timeout and sampling interval must be finite and positive")
    try:
        import psutil
    except ImportError:
        psutil = None
    peak = None
    complete = psutil is not None
    cpu_totals = {}
    timed_out = False
    disk_exhausted = False
    previous_cpu = 0.0
    previous_elapsed = 0.0
    started = time.perf_counter()
    series = Path(series_path).open("w", encoding="utf-8") if series_path else None
    process = None
    try:
        with Path(log_path).open("w", encoding="utf-8") as log:
            process = subprocess.Popen(
                [str(c) for c in command],
                stdout=log,
                stderr=subprocess.STDOUT,
                env=env,
                start_new_session=os.name == "posix",
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP
                if os.name == "nt"
                else 0,
            )
            while process.poll() is None:
                elapsed = time.perf_counter() - started
                rss, processes, handles = None, None, None
                if psutil is not None:
                    try:
                        parent = psutil.Process(process.pid)
                        members = [parent, *parent.children(recursive=True)]
                        rss, processes, handles = 0, len(members), 0
                        for member in members:
                            try:
                                rss += member.memory_info().rss
                                cpu = member.cpu_times()
                                cpu_totals[(member.pid, member.create_time())] = (
                                    cpu.user + cpu.system
                                )
                                handles += (
                                    member.num_handles()
                                    if os.name == "nt"
                                    else member.num_fds()
                                )
                            except psutil.NoSuchProcess:
                                pass
                        peak = max(peak or 0, rss)
                    except psutil.NoSuchProcess:
                        pass
                    except psutil.Error:
                        complete = False
                        rss = handles = None
                progress = None
                if progress_path:
                    try:
                        progress = json.loads(
                            Path(progress_path).read_text(encoding="utf-8")
                        )
                    except (OSError, ValueError):
                        pass  # Atomic replace can race a Windows reader.
                output_bytes = None
                if media_directory:
                    output_bytes = 0
                    for path in Path(media_directory).rglob("*"):
                        if path.suffix.lower() in {".mp4", ".tmp"}:
                            try:
                                output_bytes += path.stat().st_size
                            except FileNotFoundError:
                                pass  # Publication can rename a temporary while sampling.
                    if minimum_free_bytes is not None:
                        import shutil

                        disk_exhausted = (
                            shutil.disk_usage(media_directory).free < minimum_free_bytes
                        )
                if series:
                    cpu_seconds = sum(cpu_totals.values()) if complete else None
                    sample_seconds = elapsed - previous_elapsed
                    frame = (progress or {}).get("frame")
                    series.write(
                        json.dumps(
                            dict(
                                elapsed_seconds=elapsed,
                                tree_rss_bytes=rss,
                                cpu_seconds=cpu_seconds,
                                cpu_percent=(cpu_seconds - previous_cpu)
                                / sample_seconds
                                * 100
                                if cpu_seconds is not None and sample_seconds > 0
                                else None,
                                process_count=processes,
                                handles_or_fds=handles,
                                progress=progress,
                                average_fps=frame / elapsed
                                if isinstance(frame, (int, float)) and elapsed > 0
                                else None,
                                output_bytes=output_bytes,
                            ),
                            allow_nan=False,
                        )
                        + "\n"
                    )
                    series.flush()
                    previous_cpu = cpu_seconds or 0.0
                    previous_elapsed = elapsed
                if elapsed >= timeout or disk_exhausted:
                    timed_out = elapsed >= timeout
                    terminate_tree(process)
                    break
                time.sleep(interval)
            process.wait()
    except BaseException:
        if process is not None:
            terminate_tree(process)
        raise
    finally:
        if series:
            series.close()
    wall_ms = (time.perf_counter() - started) * 1000
    cpu_seconds = sum(cpu_totals.values()) if complete and cpu_totals else None
    return dict(
        command=[str(c) for c in command],
        returncode=process.returncode,
        timed_out=timed_out,
        disk_exhausted=disk_exhausted,
        wall_ms=wall_ms,
        peak_tree_rss_bytes=peak if complete else None,
        cpu_seconds=cpu_seconds,
        cpu_percent=cpu_seconds / (wall_ms / 1000) * 100
        if cpu_seconds is not None
        else None,
        rss_sampling_interval_ms=interval * 1000,
        memory_method="psutil simultaneous process-tree RSS" if complete else None,
        cpu_method="psutil observed process-tree CPU estimate; one core = 100%"
        if complete
        else None,
    )
