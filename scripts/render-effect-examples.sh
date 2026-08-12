#!/usr/bin/env bash
set -euo pipefail

skip_existing=false
if [[ "${1:-}" == "--skip-existing" ]]; then
  skip_existing=true
elif [[ $# -ne 0 ]]; then
  echo "usage: $0 [--skip-existing]" >&2
  exit 2
fi

cargo build --release -p vestra-cli --all-features
mkdir -p examples/output

while IFS= read -r config; do
  relative=${config#examples/}
  output="examples/output/${relative//\//-}"
  output="${output%.json}.mp4"
  if [[ "$skip_existing" == true && -f "$output" ]]; then
    echo "skip $output"
    continue
  fi
  echo "render $config -> $output"
  ./target/release/ve render "$config" \
    --render-backend cpu \
    --output "$output" \
    --progress none \
    --overwrite
done < <(find examples/effects examples/transitions examples/presets examples/compositing examples/projects -name '*.json' -type f | sort)
