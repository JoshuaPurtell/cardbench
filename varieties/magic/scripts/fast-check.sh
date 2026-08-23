#!/bin/sh
# Fast edit-loop checks. Scale campaigns remain explicit commands in README.md.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$ROOT/../.." && pwd)
TARGET_DIR=${CARGO_TARGET_DIR:-${TMPDIR:-/tmp}/cardbench-magic-target}
export CARGO_TARGET_DIR="$TARGET_DIR"
cd "$ROOT"

# `cargo fmt --all` forces Cargo metadata traversal over every package and is
# the slowest part of the edit loop.  Check the files touched by this commit
# or working tree directly; the full-workspace formatter remains an explicit
# release/CI check (`FAST_CHECK_FULL_FMT=1`).
if [ "${FAST_CHECK_FULL_FMT:-0}" = 1 ]; then
  cargo fmt --all -- --check
else
  changed_rs=$(
    {
      git -C "$REPO_ROOT" diff --name-only --diff-filter=ACMR HEAD
      git -C "$REPO_ROOT" diff --cached --name-only --diff-filter=ACMR
      git -C "$REPO_ROOT" show --format= --name-only --diff-filter=ACMR HEAD
    } | awk '/\.rs$/ { print }' | sort -u
  )
  if [ -n "$changed_rs" ]; then
    printf '%s\n' "$changed_rs" | while IFS= read -r file; do
      rustfmt --check --edition 2024 "$REPO_ROOT/$file"
    done
  fi
fi
cargo test -p cardbench-magic-engine --lib --quiet
cargo test -p cardbench-magic-policies --lib --quiet \
  reference_matrix_caps_live_workers_instead_of_spawning_one_thread_per_match
cargo test -p cardbench-magic-rav --test coverage_report_contract --quiet
cargo run -q -p cardbench-magic-policies --bin rav-engine-audit -- --quick
