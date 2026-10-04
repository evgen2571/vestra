#!/usr/bin/env python3
"""Check relative file targets in public Markdown (external URLs/anchors excluded)."""

from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit

INLINE = re.compile(r"!?\[[^\]\n]*\]\(\s*(<[^>]+>|[^\s)]+)")
REFERENCE = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*(<[^>]+>|\S+)")


def main() -> None:
    root = (
        Path(sys.argv[1]).resolve()
        if len(sys.argv) > 1
        else Path(__file__).resolve().parents[1]
    )
    files = list(root.glob("*.md"))
    for directory in ("docs", "examples", "benchmarks"):
        files.extend((root / directory).rglob("*.md"))
    checked = errors = 0
    for source in sorted(files):
        fence = None
        for number, line in enumerate(
            source.read_text(encoding="utf-8").splitlines(), 1
        ):
            marker = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
            if marker:
                token = marker[1]
                if fence is None:
                    fence = token
                elif token[0] == fence[0] and len(token) >= len(fence):
                    fence = None
                continue
            if fence is not None:
                continue
            matches = list(INLINE.finditer(line))
            reference = REFERENCE.match(line)
            if reference:
                matches.append(reference)
            for match in matches:
                target = match[1].strip("<>")
                parsed = urlsplit(target)
                if parsed.scheme or parsed.netloc or not parsed.path:
                    continue
                checked += 1
                path = unquote(parsed.path)
                destination = (
                    root / path.lstrip("/")
                    if path.startswith("/")
                    else source.parent / path
                )
                if not destination.exists():
                    print(
                        f"{source.relative_to(root)}:{number}: missing target {target}"
                    )
                    errors += 1
    print(
        f"Checked {checked} local file links in {len(files)} Markdown files; {errors} missing"
    )
    raise SystemExit(1 if errors else 0)


if __name__ == "__main__":
    main()
