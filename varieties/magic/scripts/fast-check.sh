#!/bin/sh
# Fast edit-loop checks. Scale campaigns remain explicit commands in README.md.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TARGET_DIR=${CARGO_TARGET_DIR:-${TMPDIR:-/tmp}/cardbench-magic-target}
export CARGO_TARGET_DIR="$TARGET_DIR"
cd "$ROOT"

cargo fmt --all -- --check
cargo test -p cardbench-magic-engine --lib --quiet
cargo test -p cardbench-magic-policies --lib --quiet \
  reference_matrix_caps_live_workers_instead_of_spawning_one_thread_per_match
cargo test -p cardbench-magic-rav --test scenario_index_contract --quiet
cargo run -q -p cardbench-magic-policies --bin rav-engine-audit -- --quick
