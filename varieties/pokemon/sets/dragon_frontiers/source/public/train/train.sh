#!/usr/bin/env bash
set -euo pipefail

WORKDIR="/app"
IMPORT_SPECS="$WORKDIR/scaffold/src/df/import_specs.rs"
RUNTIME="$WORKDIR/scaffold/src/df/runtime.rs"
TRAIN_RUNTIME="/tests/train/train_runtime_eval.rs"
GOLD_IMPORT_SPECS="/tests/.hidden/gold/import_specs.rs"
GOLD_RUNTIME="/tests/.hidden/gold/runtime.rs"
GENERATED_GOLD="/tmp/setbench_df_train_gold.json"

CAND_IMPORT_SPECS_BACKUP="/tmp/setbench_df_candidate_import_specs.rs"
CAND_RUNTIME_BACKUP="/tmp/setbench_df_candidate_runtime.rs"

install_eval_sources() {
  local base_import_specs="$1"
  local base_runtime="$2"

  cp "$base_import_specs" "$IMPORT_SPECS"
  cp "$base_runtime" "$RUNTIME"
  {
    echo ""
    echo "// ============================================================================"
    echo "// SETBENCH PUBLIC TRAIN TESTS (injected at runtime)"
    echo "// ============================================================================"
    echo ""
  } >> "$IMPORT_SPECS"
  cat "$TRAIN_RUNTIME" >> "$IMPORT_SPECS"
}

cleanup() {
  if [ -f "$CAND_IMPORT_SPECS_BACKUP" ]; then
    cp "$CAND_IMPORT_SPECS_BACKUP" "$IMPORT_SPECS"
  fi
  if [ -f "$CAND_RUNTIME_BACKUP" ]; then
    cp "$CAND_RUNTIME_BACKUP" "$RUNTIME"
  fi
}
trap cleanup EXIT

if [ ! -f "$IMPORT_SPECS" ] || [ ! -f "$RUNTIME" ]; then
  echo "ERROR: Missing graded files"
  exit 1
fi

if [ ! -f "$TRAIN_RUNTIME" ] || [ ! -f "$GOLD_IMPORT_SPECS" ] || [ ! -f "$GOLD_RUNTIME" ]; then
  echo "ERROR: Missing Dragon Frontiers train fixtures"
  exit 1
fi

cp "$IMPORT_SPECS" "$CAND_IMPORT_SPECS_BACKUP"
cp "$RUNTIME" "$CAND_RUNTIME_BACKUP"

install_eval_sources "$GOLD_IMPORT_SPECS" "$GOLD_RUNTIME"

cd "$WORKDIR"
SETBENCH_GOLD_OUTPUT="$GENERATED_GOLD" cargo test --package tcg_expansions setbench_eventlog_write_gold_fixture -- --ignored --nocapture

install_eval_sources "$CAND_IMPORT_SPECS_BACKUP" "$CAND_RUNTIME_BACKUP"

echo "=== SetBench Dragon Frontiers Public Train Suite ==="
echo ""
echo "=== Compile gate ==="
cargo check --package tcg_expansions

echo ""
echo "=== Public train replay ==="
SETBENCH_GOLD_PATH="$GENERATED_GOLD" cargo test --package tcg_expansions setbench_train_ -- --test-threads=1 --format=terse
