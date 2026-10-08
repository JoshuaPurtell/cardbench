#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/surge_of_zeal.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Surge of Zeal source: ${candidate}" >&2
  exit 1
fi

echo "Surge of Zeal module present; isolated Harbor scoring follows submission."
