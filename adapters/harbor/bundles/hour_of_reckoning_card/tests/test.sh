#!/bin/sh
set -eu
candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/hour_of_reckoning.rs}"
test -f "$candidate"
test -f varieties/magic/session/src/bin/rav_hour_of_reckoning_pvp.rs
test -f varieties/magic/tasks/rav-hour-of-reckoning-pvp/features.toml
echo "Hour of Reckoning source present; isolated diagnostic scoring follows submission."
