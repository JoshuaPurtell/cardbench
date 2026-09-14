#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 \$CG_PRIVATE_MODULE" >&2
  exit 2
fi

source_dir="$1"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
destination="$script_dir/../engine/scaffold/src/cg_private"

for required in mod.rs hooks.rs catalog.json cards/mod.rs; do
  if [[ ! -f "$source_dir/$required" ]]; then
    echo "private CG module is missing $required: $source_dir" >&2
    exit 1
  fi
done

if [[ -L "$source_dir" ]]; then
  echo "private CG module must be a directory, not a symlink" >&2
  exit 1
fi

mkdir -p "$destination"
find "$destination" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
cp -R "$source_dir"/. "$destination"/

if find "$destination" -type l -print -quit | grep -q .; then
  echo "installed private CG module contains a symlink" >&2
  exit 1
fi

echo "installed private CG module at $destination"
