# Lore Broker source contract

This task follows [`RAV_PVP_SOURCE_CONTRACT.md`](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md). RAV printing facts control the candidate identity and tested semantics; Oracle is consulted only to resolve interpretive questions.

- Public identity record: [Lore Broker, RAV #57](https://www.mtg.wtf/card/rav/57/Lore-Broker).
- Printed identity retained: Lore Broker, `{1}{U}`, blue Creature — Human Rogue, 1/2; `{T}: Each player draws a card, then discards a card.`
- Oracle reconciliation: current Oracle wording modernizes the discard pronoun while preserving the same operation—each player discards a card from that player's own hand. No gameplay-changing difference is used by this trace.
- Compatibility limit: `CardDefinition` currently represents `Creature` but has no creature subtype field. Human Rogue is retained here as a source fact and is not asserted as an executable characteristic.
- No scans, artwork, flavor text, or card-rules prose corpus are retained. The public record is used only to verify identity facts; behavior is represented by CardBench-authored operations.

The authority reports the Comprehensive Rules revision effective 2026-08-07 and applies the task cutoff 2026-09-24. It traces activation timing, the tap cost, ordinary draws, private hand choices, and public zone-change receipts through the game engine.
