#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

asset=examples/assets/red.png
expected_hash=1546bf939a4978f44474165e24607bb3d7542f8c84bd6c522e7249e054f4247a
actual_hash=$(sha256sum "$asset" | awk '{print $1}')

if [[ "$actual_hash" != "$expected_hash" ]]; then
  printf 'invalid example asset %s: expected %s, got %s\n' \
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
if (width, height, bit_depth, color_type, compression, filtering, interlace) != (160, 90, 1, 3, 0, 0, 0):
    raise SystemExit("red.png must be a 160x90 1-bit indexed-colour PNG")
PY
