Implement the candidate-owned Terraformer module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/terraformer.rs`.

Terraformer is RAV #70, a blue 2/2 Creature costing `{2}{U}`. Its activated ability costs `{1}` and, at instant speed, makes each land you control a basic land type of your choice until end of turn. Preserve the printed definition and declare this ability using the existing engine effect representation.

The authority requires the module to compile in the isolated RAV set crate and checks a full shuffled PvP game. The pilot casts Terraformer, activates it during the opponent's precombat main phase, changes only its own lands to Forests, and gets Green from a former Island. It activates Terraformer a second time in its own precombat main phase, uses the converted Islands to pay `{3}{G}` for Greater Mossdog, then continues to a terminal win. The trace checks each payment, source/incarnation, chosen type, exact per-land effects and expiration, unchanged opposing lands, restoration, spell resolution, ordered receipts, deterministic replay, and semantic mutations.

Keep behavior in the declarative candidate-owned module. Do not edit shared registry, engine, authority, scoring, or task files. The evaluator grants no card credit or reward.
