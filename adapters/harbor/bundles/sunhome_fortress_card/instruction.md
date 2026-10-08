# CardBench Magic single-card task: Sunhome, Fortress of the Legion

Implement the RAV #282 land in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/sunhome_fortress_of_the_legion.rs`.
Keep the source to the definition, colorless mana binding, and targeted
Double Strike binding as declarative constructors.

The trusted verifier runs a shuffled two-player constructed match. It requires
Sunhome to produce colorless mana for a real creature cast, then to pay the
exact `{2}{R}{W}` and tap cost while targeting a creature. That creature must
gain Double Strike, attack after the ability resolves, and deal both combat
damage packets before the match ends. The verifier compares a full replay and
checks semantic mutations. The task does not award card credit or Harbor
reward.
