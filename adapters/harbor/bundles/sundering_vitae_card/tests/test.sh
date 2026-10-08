#!/bin/sh
set -eu
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/sundering_vitae.rs}"
test -f "$candidate"
test -f varieties/magic/session/src/bin/rav_sundering_vitae_pvp.rs
test -f varieties/magic/tasks/rav-sundering-vitae-pvp/features.toml
echo "Sundering Vitae source present; isolated diagnostic scoring follows submission."
