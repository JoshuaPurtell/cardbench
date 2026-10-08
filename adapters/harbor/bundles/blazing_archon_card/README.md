# CardBench Magic single-card task: Blazing Archon

Implement Blazing Archon at RAV collector #4 in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/blazing_archon.rs`.
Keep the candidate-owned pure definition and static attack-restriction binding.

The legal PvP trace pays `{6}{W}{W}{W}`, resolves a white 5/6 creature with
Flying, observes a legal opposing attack before entry, and then requires the
opponent's typed attacker declaration to be empty while Archon is live. The
Archon itself attacks to a terminal win. The authority replays the seed and
detects compiled mutations to Flying and the restriction binding.

This is a diagnostic-only pilot with zero set coverage, card credit, and reward.
