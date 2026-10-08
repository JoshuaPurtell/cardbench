#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/remand.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Remand source: ${candidate}" >&2
  exit 1
fi
echo "Remand source present; isolated Harbor scoring follows submission."
