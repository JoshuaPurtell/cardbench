#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/stone_seeder_hierophant.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Stone-Seeder Hierophant source: ${candidate}" >&2
  exit 1
fi

echo "Stone-Seeder Hierophant source present; isolated Harbor scoring follows submission."
