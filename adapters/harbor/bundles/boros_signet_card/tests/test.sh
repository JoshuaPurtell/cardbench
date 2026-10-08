#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_signet.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Boros Signet source: ${candidate}" >&2
  exit 1
fi

echo "Boros Signet source present; isolated Harbor scoring follows submission."
