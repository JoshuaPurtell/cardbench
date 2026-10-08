#!/bin/sh
set -eu
test -f varieties/magic/sets/ravnica_city_of_guilds/src/cards/dark_heart_of_the_wood.rs
test -f varieties/magic/session/src/bin/rav_dark_heart_of_the_wood_pvp.rs
test -f varieties/magic/tasks/rav-dark-heart-of-the-wood-pvp/features.toml
