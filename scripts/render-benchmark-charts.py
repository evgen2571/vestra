#!/usr/bin/env python3
"""Plot the published Markdown measurements; never collect new timings.

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
    plt.rcParams.update(
        {"svg.fonttype": "none", "svg.hashsalt": "vestra", "font.size": 11}
    )
    context = next(
        line.removeprefix("<!-- Chart context: ").removesuffix(" -->")
        for line in source.splitlines()
        if line.startswith("<!-- Chart context: ")
    )
    matched = [
        r for r in rows if r[0] in {"basic", "editing", "layered"} and len(r) == 12
    ]
    effect_names = (
        "stylization_baseline",
        "palette_video",
        "dither_video",
        "ascii_video",
        "halftone_video",
        "sort_horizontal_video",
        "sort_vertical_video",
        "crt_video",
    )
    by_name = {r[1]: r for r in rows if len(r) == 8 and r[1] in effect_names}
    effects = [by_name[name] for name in effect_names]
    for filename, labels, series in (
        (
            "moviepy-times",
            ["Basic export", "Three-shot edit", "Layered"],
            [
                ("MoviePy CPU", [float(r[1]) for r in matched], "#bdc9d3", ""),
                ("Vestra CPU", [float(r[2]) for r in matched], "#7194b3", "//"),
                ("Vestra GPU", [float(r[4]) for r in matched], "#4e5d6a", ".."),
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
                ("Vestra CPU", [float(r[3]) for r in effects], "#7194b3", "//"),
                ("Vestra GPU", [float(r[5]) for r in effects], "#4e5d6a", ".."),
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
        ax.set_title(context, fontsize=10, pad=48)
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
                "Description": "Measured medians from docs/benchmarks/vestra-vs-moviepy.md; see that report for capture context.",
            },
        )
        plt.close(fig)
        svg = target.read_text(encoding="utf-8")
        palette = {
            "#ffffff": "bg",
            "#000000": "ink",
            "#ededed": "line",
            "#bdc9d3": "moviepy",
            "#7194b3": "cpu",
            "#4e5d6a": "gpu",
        }
        for color, token in palette.items():
            svg = svg.replace(color, f"var(--{token})")
        style = """<style>
:root { --bg:#f0f2f4; --ink:#242b33; --line:#c9d1d9; --moviepy:#bdc9d3; --cpu:#7194b3; --gpu:#4e5d6a; }
@media (prefers-color-scheme:dark) { :root { --bg:#292e35; --ink:#eef2f6; --line:#4e5d6a; --moviepy:#bdc9d3; --cpu:#83a5c2; --gpu:#95a7b6; } }
text { fill:var(--ink); font-family:system-ui,sans-serif; }
</style>
"""
        svg = svg.replace(" <defs>", style + " <defs>", 1)
        target.write_text(
            "\n".join(line.rstrip() for line in svg.splitlines()) + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
