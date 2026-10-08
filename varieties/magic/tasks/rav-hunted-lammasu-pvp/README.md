# Hunted Lammasu full-game card task

Implement the candidate-owned Hunted Lammasu definition and triggered ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_lammasu.rs`. Hunted Lammasu is a white `{2}{W}{W}` 5/5 creature with flying. Its mandatory enters-the-battlefield ability targets an opponent, who creates one black 4/4 Horror creature token.

The trusted authority runs a shuffled 60-card two-player game. It checks exact mana payment, the live 5/5 Flying characteristics, selection of the opponent, the token's controller and full characteristics, trigger/source-incarnation resolution, and a terminal P0 combat win. The trace is replayed deterministically and gameplay mutations cover flying, trigger presence, opponent-only targeting, token identity, and mana cost.

This is a diagnostic task over 291 distinct RAV names and 306 printings. It grants zero card credit and zero reward.
