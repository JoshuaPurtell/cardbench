# Junktroller full-game card task

Implement RAV Junktroller (#264) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/junktroller.rs`. The task follows the original RAV #264 printed card: a colorless `{4}` 0/6 artifact creature with defender and `{T}: Put target card from a graveyard on the bottom of its owner's library.` The original-card scan in the Star City Games RAV #264 listing shows no mana symbol before the tap symbol. The ability has no shuffle instruction.

The trusted authority runs a deterministic, normally shuffled two-player match using legal 60-card decks. Junktroller is cast and used to target an opponent-owned instant in that opponent's graveyard. The trace verifies that the ability has no mana payment, that Junktroller taps, the target identity and owner, the target's move to the bottom of its owner's library, preservation of the remaining library order, and the absence of a shuffle receipt. Both players continue through ordinary play until a terminal PvP result.

Printed-text provenance: [Star City Games Ravnica: City of Guilds #264 listing](https://starcitygames.com/junktroller-sgl-mtg-rav-264-enf/) displays the original card scan used to confirm the tap-only cost.

The diagnostic authority checks deterministic replay and semantic mutations for the cast cost, defender, activation cost, tap requirement, target scope, and library-bottom effect. This is one uncredited diagnostic identity within the 291-name / 306-printing RAV denominator. It is not a provider trial or card credit.

Materialize with `materialize_junktroller_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
