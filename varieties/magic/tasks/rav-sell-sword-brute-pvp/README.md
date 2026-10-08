# Sell-Sword Brute RAV card task

Implement the candidate-owned module for Sell-Sword Brute, RAV collector #142. It is a red `{1}{R}` 2/2 creature whose mandatory, untargeted dies trigger deals two damage to its controller.

The authority runs a legal, shuffled 60-card two-player game. Seat one pays `{1}{R}` to cast Sell-Sword Brute. Seat zero pays `{1}{B}` for Last Gasp and resolves it against the Brute; the toughness-zero state action moves it to its owner's graveyard, after which its dies trigger resolves and deals two damage to its controller. The same game continues to a terminal seat-zero win. The trace checks ordered typed events, trigger source incarnation, actual colored payments, deterministic replay, and semantic mutants that remove the trigger or change its damage to one.

The source contract follows the RAV printing represented in the set catalog and `RAV_PVP_SOURCE_CONTRACT.md`. The task retains only a CardBench-authored semantic summary and public identity fields. Rules baseline: the Comprehensive Rules effective August 7, 2026, as of the September 23, 2026 authority cutoff. No provider trial is run during local package checks.
