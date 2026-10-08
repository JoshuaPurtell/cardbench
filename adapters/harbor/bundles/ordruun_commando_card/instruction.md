# Candidate work

Implement the candidate-owned Ordruun Commando definition and its `{W}` prevention ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/ordruun_commando.rs`. Preserve the RAV #137 identity, red `{3}{R}` cast, 4/1 creature characteristics, and `{W}` ability that creates a one-point shield on this creature until end of turn. The set registry delegates both the card definition and its activated binding to this module.

The isolated authority runs an ordinary shuffled two-player game and requires a live combat interaction: the opposing Elves of Deep Shadow attacks, Commando blocks, the shield prevents exactly one damage before the Commando deals lethal combat damage back, then Commando attacks later and wins. Keep the module declarative and within the literal source contract. The scorer validates the full ordered gameplay evidence and targeted mutations.
