#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/searing_meditation.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Searing Meditation source: ${candidate}" >&2
  exit 1
fi

echo "Searing Meditation source present; isolated Harbor scoring follows submission."
