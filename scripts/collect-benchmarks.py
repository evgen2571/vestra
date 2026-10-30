# /// script
# requires-python = ">=3.11,<3.15"
# dependencies = []
# ///
"""Collect portable Vestra benchmark evidence; bootstrap uses only the stdlib."""

from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from benchmarks.collector.collect import main

if __name__ == "__main__":
    sys.exit(main())
