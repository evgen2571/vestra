#!/usr/bin/env python3
"""Plot the historical Markdown measurements; never collect new timings.

uv run --no-project --with matplotlib==3.10.7 python scripts/render-benchmark-charts.py
"""

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]


def main():
    source = (ROOT / "docs/benchmarks/vestra-vs-moviepy.md").read_text(encoding="utf-8")
    rows = [
        [cell.strip() for cell in line.strip("|").split("|")]
        for line in source.splitlines()
        if line.startswith("| ")
    ]
    plt.rcParams.update({"svg.fonttype": "none", "font.size": 10})
    matched = [
        r for r in rows if r[0] in {"basic", "editing", "layered"} and len(r) == 12
    ]
    effects = [r for r in rows if r[0] == "stylized-effects-1080p" and len(r) == 8]
    for filename, labels, series in (
        (
            "moviepy-times",
            ["Basic export", "Three-shot edit", "Layered"],
            [
                ("MoviePy CPU", [float(r[1]) for r in matched], "#939393", ""),
                ("Vestra CPU", [float(r[2]) for r in matched], "#326a8d", "//"),
                ("Vestra GPU", [float(r[4]) for r in matched], "#b35b35", ".."),
            ],
        ),
        (
            "effect-times",
            [
                "No effect",
                "Palette",
                "Bayer dither",
                "ASCII",
                "Halftone",
                "Horizontal sort",
                "Vertical sort",
                "CRT",
            ],
            [
                ("Vestra CPU", [float(r[3]) for r in effects], "#326a8d", "//"),
                ("Vestra GPU", [float(r[5]) for r in effects], "#b35b35", ".."),
            ],
        ),
    ):
        assert all(len(values) == len(labels) for _, values, _, _ in series)
        fig, ax = plt.subplots(
            figsize=(5.2, 1.6 + len(labels) * 0.7), layout="constrained"
        )
        width = 0.22 if len(series) == 3 else 0.3
        maximum = max(max(values) for _, values, _, _ in series)
        for index, (name, values, color, hatch) in enumerate(series):
            y = [
                n + (index - (len(series) - 1) / 2) * width for n in range(len(labels))
            ]
            bars = ax.barh(
                y,
                values,
                height=width * 0.88,
                label=name,
                color=color,
                hatch=hatch,
                edgecolor="white",
                linewidth=0.5,
            )
            ax.bar_label(
                bars, labels=[f"{v:.3f}" for v in values], padding=3, fontsize=9
            )
        ax.set_yticks(range(len(labels)), labels)
        ax.invert_yaxis()
        ax.set_xlim(0, maximum * 1.24)
        ax.set_xlabel("Render time (seconds) · lower is better")
        ax.set_title("31 Oct 2026 · Ryzen 5 3600 / GTX 1650 SUPER", fontsize=10, pad=48)
        ax.legend(
            loc="lower center",
            bbox_to_anchor=(0.5, 1.02),
            ncol=len(series),
            frameon=False,
            fontsize=9,
        )
        ax.spines[["top", "right", "left"]].set_visible(False)
        ax.tick_params(axis="y", length=0)
        ax.set_axisbelow(True)
        ax.xaxis.grid(True, color="#ededed")
        target = ROOT / f"docs/assets/benchmarks/{filename}.svg"
        fig.savefig(
            target,
            metadata={
                "Date": None,
                "Description": "Historical medians from docs/benchmarks/vestra-vs-moviepy.md; not current performance claims.",
            },
        )
        plt.close(fig)
        target.write_text(
            "\n".join(
                line.rstrip()
                for line in target.read_text(encoding="utf-8").splitlines()
            )
            + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
