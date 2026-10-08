# Frenzied Goblin RAV card task

Implement the candidate-owned module for Frenzied Goblin, RAV collector #125. It is a red `{R}` 1/1 creature with an optional red attack trigger that can keep a targeted creature from blocking for the turn.

The trusted authority runs legal, shuffled 60-card decks through a full two-player match. Seat zero pays `{R}` to cast Frenzied Goblin; seat one pays `{G}{W}` for a real Watchwolf. On a later attack, the Goblin's optional trigger is stacked, its controller produces and pays red mana, and the controller selects the opposing Watchwolf as a legal creature target. The resolved effect prevents Watchwolf from blocking, so the blocker declaration is empty and the Goblin deals combat damage. The match continues to a terminal seat-zero win. The report checks typed event order, target identity, source incarnation, deterministic event-log replay, and gameplay mutations that remove the trigger or replace its blocking restriction.

The source contract follows the RAV printing represented in the catalog and `RAV_PVP_SOURCE_CONTRACT.md`. The task retains only public identity fields and a CardBench-authored semantic summary. Rules baseline: the Comprehensive Rules effective August 7, 2026, as of the September 23, 2026 authority cutoff. Local package checks do not run Harbor or a provider trial.
