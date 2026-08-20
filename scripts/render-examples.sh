#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

usage() {
  cat <<'EOF'
usage:
  scripts/render-examples.sh [--validate-only] [--all] [--category CATEGORY] [--output-dir DIR]

Validates every canonical JSON example with `ve validate`. By default it also
renders one representative from each category. `--all` renders every example;
`--category` renders every example in one category. Outputs go to a temporary
directory unless `--output-dir` is supplied.

Categories: effects, transitions, presets, compositing, projects, particles
EOF
}

categories=(effects transitions presets compositing projects particles)
validate_only=false
render_all=false
category=""
output_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --validate-only)
      validate_only=true
      shift
      ;;
    --all)
      render_all=true
      shift
      ;;
    --category)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      category=$2
      shift 2
      ;;
    --output-dir)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      output_dir=$2
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -n "$category" ]]; then
  valid_category=false
  for candidate in "${categories[@]}"; do
    if [[ "$candidate" == "$category" ]]; then
      valid_category=true
      break
    fi
  done
  if [[ "$valid_category" == false ]]; then
    echo "unknown example category: $category" >&2
    exit 2
  fi
fi

mapfile -t examples < <(find "${categories[@]/#/examples/}" -type f -name '*.json' | sort)
if [[ ${#examples[@]} -eq 0 ]]; then
  echo "no canonical JSON examples found" >&2
  exit 1
fi

cargo build -q -p vestra-cli --all-features
for example in "${examples[@]}"; do
  echo "validate $example"
  target/debug/ve validate "$example" --format human
done

if [[ "$validate_only" == true ]]; then
  echo "validated ${#examples[@]} canonical JSON examples"
  exit 0
fi

declare -a selected=()
if [[ -n "$category" ]]; then
  for example in "${examples[@]}"; do
    [[ "$example" == "examples/$category/"* ]] && selected+=("$example")
  done
elif [[ "$render_all" == true ]]; then
  selected=("${examples[@]}")
else
  for current_category in "${categories[@]}"; do
    for example in "${examples[@]}"; do
      if [[ "$example" == "examples/$current_category/"* ]]; then
        selected+=("$example")
        break
      fi
    done
  done
fi

if [[ ${#selected[@]} -eq 0 ]]; then
  echo "no examples selected for rendering" >&2
  exit 1
fi

cleanup_output=false
if [[ -z "$output_dir" ]]; then
  output_dir=$(mktemp -d -t vestra-examples-XXXXXX)
  cleanup_output=true
  trap 'rm -rf -- "$output_dir"' EXIT
else
  mkdir -p "$output_dir"
fi

for example in "${selected[@]}"; do
  relative=${example#examples/}
  output="$output_dir/${relative//\//-}"
  output="${output%.json}.mp4"
  echo "render $example -> $output"
  target/debug/ve render "$example" \
    --render-backend cpu \
    --output "$output" \
    --progress none \
    --overwrite
done

if [[ "$cleanup_output" == true ]]; then
  echo "rendered ${#selected[@]} representative examples in a temporary directory"
else
  echo "rendered ${#selected[@]} examples in $output_dir"
fi
