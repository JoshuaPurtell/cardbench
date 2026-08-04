#!/bin/sh
# Run deterministic, bounded slices of the Magic test inventory.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TARGET_DIR=${CARGO_TARGET_DIR:-${TMPDIR:-/tmp}/cardbench-magic-target}
export CARGO_TARGET_DIR="$TARGET_DIR"

usage() {
  cat <<'EOF'
usage:
  ./scripts/check-batch.sh list
  ./scripts/check-batch.sh core
  ./scripts/check-batch.sh policies
  ./scripts/check-batch.sh protocol [test|clippy]
  ./scripts/check-batch.sh session [test|clippy]
  ./scripts/check-batch.sh ladder [SEED_PAIRS]
  ./scripts/check-batch.sh stats [SEEDS]
  ./scripts/check-batch.sh matrix [SEED_PAIRS]
  ./scripts/check-batch.sh rav SHARD[/TOTAL] [test|clippy]
  ./scripts/check-batch.sh engine SHARD[/TOTAL] [test|clippy]

Examples:
  ./scripts/check-batch.sh rav 1/48
  ./scripts/check-batch.sh rav 2/48 clippy
  ./scripts/check-batch.sh engine 1/32

Test targets are sorted by name and assigned round-robin, so every shard is
stable across machines. Defaults are 48 RAV shards and 32 engine shards.
EOF
}

run_shard() {
  package=$1
  test_dir=$2
  shard_spec=$3
  default_total=$4
  action=$5

  case "$shard_spec" in
    */*)
      shard=${shard_spec%/*}
      total=${shard_spec#*/}
      ;;
    *)
      shard=$shard_spec
      total=$default_total
      ;;
  esac
  case "$shard" in ''|*[!0-9]*) usage; exit 2 ;; esac
  case "$total" in ''|*[!0-9]*|0) usage; exit 2 ;; esac
  if [ "$shard" -lt 1 ] || [ "$shard" -gt "$total" ]; then
    printf >&2 'shard must be in the inclusive range 1..%s\n' "$total"
    exit 2
  fi
  case "$action" in
    test) set -- cargo test --quiet -p "$package" ;;
    clippy) set -- cargo clippy --quiet -p "$package" ;;
    *) usage; exit 2 ;;
  esac

  index=0
  selected=0
  for path in $(find "$test_dir" -maxdepth 1 -type f -name '*.rs' | LC_ALL=C sort); do
    index=$((index + 1))
    if [ $(((index - 1) % total + 1)) -eq "$shard" ]; then
      target=$(basename "$path" .rs)
      set -- "$@" --test "$target"
      selected=$((selected + 1))
    fi
  done
  if [ "$selected" -eq 0 ]; then
    printf >&2 'shard %s/%s selects no test targets\n' "$shard" "$total"
    exit 2
  fi

  printf 'batch package=%s action=%s shard=%s/%s targets=%s\n' \
    "$package" "$action" "$shard" "$total" "$selected"
  cd "$ROOT"
  if [ "$action" = clippy ]; then
    "$@" -- -D warnings
  else
    "$@"
  fi
}

command=${1:-list}
case "$command" in
  list)
    printf '%s\n' \
      '1. core       formatting + engine lib + policy boundary + coverage + quick audit' \
      '2. policies   all policy library tests (scale campaigns remain explicit)' \
      '3. protocol   transport schema, staleness, and redaction contract tests' \
      '4. session    engine-to-protocol projection and transcript fidelity tests' \
      '5. ladder     policy generation N vs N-1 across every constructed deck' \
      '6. matrix     constructed-deck matchup matrix with confidence intervals' \
      '6b. stats     per-deck campaign statistics: what the pilots actually did' \
      '7. rav        deterministic slice of RAV integration-test targets' \
      '8. engine     deterministic slice of engine integration-test targets' \
      '9. exhaustive workspace tests/clippy (release-only; not run by this script)'
    ;;
  core)
    cd "$ROOT"
    ./scripts/core-check.sh
    ;;
  policies)
    cd "$ROOT"
    cargo test --quiet -p cardbench-magic-policies --lib
    ;;
  protocol)
    # The protocol crate is small and has no engine dependency, so it runs
    # whole rather than sharded.
    cd "$ROOT"
    case "${2:-test}" in
      test) cargo test --quiet -p cardbench-magic-protocol ;;
      clippy) cargo clippy --quiet -p cardbench-magic-protocol --all-targets -- -D warnings ;;
      *) usage; exit 2 ;;
    esac
    ;;
  session)
    # Transcript projection and fidelity. Small and unsharded, like protocol.
    cd "$ROOT"
    case "${2:-test}" in
      test) cargo test --quiet -p cardbench-magic-session ;;
      clippy) cargo clippy --quiet -p cardbench-magic-session --all-targets -- -D warnings ;;
      *) usage; exit 2 ;;
    esac
    ;;
  ladder)
    # Both seats play the same deck and differ only in policy generation, so a
    # win rate is attributable to the policy change and nothing else. Release
    # profile: a debug build makes this campaign roughly ten times slower.
    cd "$ROOT"
    cargo run --release --quiet -p cardbench-magic-policies --bin rav-policy-ladder \
      -- "${2:-25}"
    ;;
  stats)
    cd "$ROOT"
    cargo run --release --quiet -p cardbench-magic-session --bin rav-campaign-stats \
      -- "${2:-30}"
    ;;
  matrix)
    cd "$ROOT"
    cargo run --release --quiet -p cardbench-magic-policies --bin rav-archetype-matrix \
      -- "${2:-25}"
    ;;
  rav)
    run_shard cardbench-magic-rav \
      "$ROOT/sets/ravnica_city_of_guilds/tests" "${2:-1/48}" 48 "${3:-test}"
    ;;
  engine)
    run_shard cardbench-magic-engine \
      "$ROOT/engine/tests" "${2:-1/32}" 32 "${3:-test}"
    ;;
  *) usage; exit 2 ;;
esac
