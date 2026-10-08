# CardBench Magic single-card task: Flight of Fancy

Implement Flight of Fancy in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/flight_of_fancy.rs`. Provide direct definition and triggered-ability literals for its creature Aura attachment, flying grant, and ETB draw-two ability.

The trusted authority runs a shuffled 60-card two-player full game. It casts and attaches Flight of Fancy to a controlled creature, verifies the flying effect and two-card draw through typed public events, then continues through combat to a terminal result. Deterministic replay and compiled semantic mutations are required. The diagnostic reports one passed identity among 291 RAV names and 306 printings with zero card credit and zero reward.
