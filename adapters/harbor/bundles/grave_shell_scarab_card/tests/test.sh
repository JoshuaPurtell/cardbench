#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/grave_shell_scarab.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Grave-Shell Scarab source: ${candidate}" >&2
  exit 1
fi
echo "Grave-Shell Scarab candidate module present; the isolated authority covers self-Dredge and declined-draw behavior."
