# RAV Selesnya Signet PvP task

Implement the candidate-owned Selesnya Signet definition and paid green-white mana ability. In a legal 60-card PvP game, cast and resolve the colorless `{2}` artifact, pay `{1}` and tap it to create one green and one white mana in a non-stack bundle, then spend those exact colors to cast Watchwolf for `{G}{W}`. The trace checks Watchwolf as a live vanilla 3/3, waits until a later turn to attack, reaches a terminal player-zero win, and reproduces the full event stream on deterministic replay.

The authority also changes the mana output to a wrong color pair and requires the live payment trace to reject it. The materialized compile-valid starter stub has the same semantic defect. Only the candidate-owned Selesnya Signet module is editable; Watchwolf is a reviewed support card. The task uses the shared Magic workspace filter and does not run a provider trial.
