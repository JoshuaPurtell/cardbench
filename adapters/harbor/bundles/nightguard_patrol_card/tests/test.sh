#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/nightguard_patrol.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Nightguard Patrol source: ${candidate}" >&2
  exit 1
fi

echo "Nightguard Patrol source present; isolated Harbor scoring follows submission."
