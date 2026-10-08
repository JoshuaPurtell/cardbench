#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/zephyr_spirit.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Zephyr Spirit source: ${candidate}" >&2
  exit 1
fi

echo "Zephyr Spirit source present; isolated Harbor scoring follows submission."
