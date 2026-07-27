#!/usr/bin/env bash
set -euo pipefail

asset=examples/assets/red.png
expected_hash=2cfe142de2e6fc4df682c6c77a75011c0120476ce65e0c79f8be2895d6428037
actual_hash=$(sha256sum "$asset" | awk '{print $1}')

if [[ "$actual_hash" != "$expected_hash" ]]; then
  printf 'invalid public example asset %s: expected %s, got %s\n' \
    "$asset" "$expected_hash" "$actual_hash" >&2
  exit 1
fi

python3 - "$asset" <<'PY'
from pathlib import Path
import struct
import sys

data = Path(sys.argv[1]).read_bytes()
if data[:8] != b"\x89PNG\r\n\x1a\n":
    raise SystemExit("red.png is not a PNG")
width, height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(
    ">IIBBBBB", data[16:29]
)
if (width, height, bit_depth, color_type, compression, filtering, interlace) != (800, 2778, 8, 6, 0, 0, 0):
    raise SystemExit("red.png must be an 800x2778 RGBA PNG")
PY
