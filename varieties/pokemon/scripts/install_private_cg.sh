#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /path/to/private/cg" >&2
  exit 2
fi

source_dir="${1%/}"
script_dir="$(cd "$(dirname "$0")" && pwd)"
target_dir="$script_dir/../engine/scaffold/src/cg_private"

for required in mod.rs hooks.rs catalog.json cards/mod.rs; do
  if [[ ! -f "$source_dir/$required" ]]; then
    echo "private CG module is missing $required" >&2
    exit 2
  fi
done

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/cardbench-cg-private.XXXXXX")"
trap 'rm -rf "$temp_dir"' EXIT
cp -R "$source_dir/." "$temp_dir/"
rm -rf "$target_dir"
mkdir -p "$(dirname "$target_dir")"
mv "$temp_dir" "$target_dir"
trap - EXIT
echo "installed private CG module at $target_dir"
