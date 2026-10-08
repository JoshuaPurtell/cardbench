# RAV Dimir Signet PvP task

Implement the candidate-owned Dimir Signet definition and paid blue-black mana ability. In a legal 60-card PvP game, cast and resolve the colorless `{2}` artifact, pay `{1}` and tap it to create one blue and one black mana in a non-stack bundle, then spend those exact colors to cast Dimir Infiltrator for `{U}{B}`. The trace checks the live 1/3 support creature and its printed unblockable and transmute characteristics, later-turn combat, a terminal player-zero win, ordered events, and deterministic replay.

The authority changes the mana output to a wrong color pair and requires the live payment trace to reject it. The compile-valid starter stub has the same semantic defect. Only the candidate-owned Dimir Signet module is editable; Dimir Infiltrator is a reviewed support card. The task uses the shared Magic workspace filter and does not run a provider trial.
