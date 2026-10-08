#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/disembowel.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Disembowel candidate source: ${candidate}" >&2
  exit 1
fi
echo "Disembowel source present; isolated Harbor scoring follows submission."
