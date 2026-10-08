#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/chant_of_vitu_ghazi.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Chant of Vitu-Ghazi source: ${candidate}" >&2
  exit 1
fi

echo "Chant of Vitu-Ghazi source present; isolated Harbor scoring follows submission."
