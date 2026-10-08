#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/gaze_of_the_gorgon.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Gaze of the Gorgon source: ${candidate}" >&2
  exit 1
fi
echo "Gaze of the Gorgon source present; isolated Harbor scoring follows submission."
