#!/bin/sh
set -eu
test -f varieties/magic/sets/ravnica_city_of_guilds/src/cards/trophy_hunter.rs
test -f varieties/magic/session/src/bin/rav_trophy_hunter_task_pvp.rs
test -f varieties/magic/tasks/rav-trophy-hunter-pvp/features.toml
