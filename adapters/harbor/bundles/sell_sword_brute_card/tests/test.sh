#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/sell_sword_brute.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Sell-Sword Brute source: ${candidate}" >&2
  exit 1
fi
echo "Sell-Sword Brute source present; isolated Harbor scoring follows submission."
