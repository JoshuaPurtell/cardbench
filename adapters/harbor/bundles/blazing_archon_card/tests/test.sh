#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/blazing_archon.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Blazing Archon source: ${candidate}" >&2
  exit 1
fi

echo "Blazing Archon source present; isolated Harbor scoring follows submission."
