#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
leaks="$(git -C "$repo_root" ls-files | grep -E '(^|/)cg_private(/|$)|(^|/)cards/cg_0[0-9][0-9]\.rs$' || true)"
if [[ -n "$leaks" ]]; then
  echo "private CG implementation paths are tracked:" >&2
  echo "$leaks" >&2
  exit 1
fi
echo "no private CG implementation paths are tracked"
