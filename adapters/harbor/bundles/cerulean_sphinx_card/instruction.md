# CardBench Magic single-card task: Cerulean Sphinx

Implement Cerulean Sphinx in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/cerulean_sphinx.rs`. Return direct definition and activated-ability literals matching its `{4}{U}{U}` cost, blue 5/5 Creature type line, flying, and `{U}` owner-library shuffle ability.

The trusted diagnostic authority runs a shuffled 60-card two-player full game. It casts Cerulean Sphinx, uses Flash Conscription to change its controller, then activates the Sphinx under that player's control. Verify that the exact source returns to its owner's library and that only the owner's library is shuffled. The game then proceeds through combat to a terminal PvP result. Deterministic replay and compiled semantic mutations are required.
