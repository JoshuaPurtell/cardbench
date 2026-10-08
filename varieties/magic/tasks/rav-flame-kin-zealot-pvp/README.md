# Flame-Kin Zealot full-game card task

Implement RAV Flame-Kin Zealot (#206) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/flame_kin_zealot.rs`. The printed identity is a red-white `{1}{R}{R}{W}` 2/2 Creature — Elemental Berserker. Its mandatory, target-free enters-the-battlefield trigger gives creatures its controller controls +1/+1 and Haste until end of turn. The current `CardDefinition` model has no subtype field, so the subtype line is retained in source metadata and is not asserted by executable state.

The reference authority replaces the inline RAV registry entries locally with the candidate-owned definition and trigger binding, then runs an ordinary shuffled two-player duel with legal 60-card decks. It records the exact spell payment, battlefield entry, trigger stack and resolution, and creature characteristics on both sides. The Zealot and a pre-existing allied creature must receive the team bonus; a newly entered Zealot must attack in the same turn after its trigger resolves. The full game continues to a terminal PvP result and deterministic replay.

The authority detects gameplay mutations to the printed cost and body, missing or mistimed trigger behavior, the pump amount, and the Haste grant. Its normalized literal-source and AST guard runs before either candidate function; a compile-valid side-effect injection must be rejected by that guard. The diagnostic reports one passing RAV identity among 291 names and 306 printings, with zero card credit and zero reward. No provider or Harbor trial is included in local reference work.

The RAV printing uses the legacy phrase “comes into play”; current Oracle uses “enters.” This is a terminology update only. See [source-note.md](source-note.md) for the reconciliation and source links.
