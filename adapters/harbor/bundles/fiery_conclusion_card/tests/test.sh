#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/fiery_conclusion.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Fiery Conclusion source: ${candidate}" >&2
  exit 1
fi

echo "Fiery Conclusion module present; isolated Harbor scoring follows submission."
