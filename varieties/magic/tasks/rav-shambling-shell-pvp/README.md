# Shambling Shell RAV full-game task

Implement Shambling Shell (#230) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/shambling_shell.rs`. The RAV printing is a black-green `{1}{B}{G}` 3/1 Creature — Plant Zombie with the printed activated ability “Sacrifice Shambling Shell: Put a +1/+1 counter on target creature”, and Dredge 3.

The trusted authority checks both card mechanics in a legal deterministic PvP game: activate the ability, verify the Shell is sacrificed as its cost and the chosen creature receives exactly one +1/+1 counter, then use Dredge 3 to replace an actual draw with exactly three cards milled and the same Shell object returned from the graveyard to hand. It replays the game, checks semantic mutations and source rejection, and continues to a terminal result.

Plant and Zombie are retained as printed identity only: `CardDefinition` currently has no creature-subtype field. The task is diagnostic only; it awards zero card credit and zero reward. See [source-note.md](source-note.md) for the RAV/Oracle reconciliation and model boundary.
