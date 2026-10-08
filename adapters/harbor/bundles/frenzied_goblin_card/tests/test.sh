#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/frenzied_goblin.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Frenzied Goblin source: ${candidate}" >&2
  exit 1
fi
echo "Frenzied Goblin source present; isolated Harbor scoring follows submission."
