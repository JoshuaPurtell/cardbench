#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/goblin_spelunkers.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Goblin Spelunkers source: ${candidate}" >&2
  exit 1
fi

echo "Goblin Spelunkers source present; isolated Harbor scoring follows submission."
