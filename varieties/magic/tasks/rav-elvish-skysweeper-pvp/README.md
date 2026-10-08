# Elvish Skysweeper RAV full-game task

Implement Elvish Skysweeper (#162) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/elvish_skysweeper.rs`. Keep its definition and activated-ability binding in that candidate-owned module. The printed card is a green `{G}` 1/1 Creature with “`{4}{G}, Sacrifice a creature: Destroy target creature with flying.`”

The authority plays two normally shuffled, legal 60-card decks. The pilot casts Skysweeper and a Birds of Paradise, sacrifices that exact Bird to activate the ability, and destroys the opponent's exact flying Birds of Paradise while an opposing Watchwolf remains as a nonflying target decoy. It continues combat to a terminal pilot win.

The reference authority validates the pinned candidate-module AST before calling either candidate factory, rejects a compile-valid side-effect injection, and then validates the printed body and costs, exactly one creature sacrifice, the flying-only target and destroy effect, exact source/sacrifice/target object identities, ordered cost/activation/destruction/terminal receipts, deterministic replay, and five gameplay mutations. It awards zero card credit and zero reward. The public workspace permits changes only to the declared candidate module.

The original RAV printing identifies the creature as an Elf Warrior. That subtype is retained as metadata in `features.toml`; the current runtime card model asserts only `Creature`. See [`source-note.md`](source-note.md) for the printed-versus-Oracle reconciliation and sources.

Run the local reference authority with:

```sh
CARDBENCH_CARD_TASK_OUTPUT_ROOT=/tmp/rav-elvish-skysweeper-reference \
  cargo run --offline --locked --manifest-path varieties/magic/session/Cargo.toml \
  --bin rav-elvish-skysweeper-pvp
```

The candidate workspace is materialized with `varieties/magic/scripts/materialize_elvish_skysweeper_task.py`. `varieties/magic/scripts/score_elvish_skysweeper_pvp.py` is the isolated offline candidate scorer. No Harbor/provider trial is part of the reference gates.
