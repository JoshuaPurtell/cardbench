#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/veteran_armorer.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Veteran Armorer source: ${candidate}" >&2
  exit 1
fi

echo "Veteran Armorer source present; isolated Harbor scoring follows submission."
