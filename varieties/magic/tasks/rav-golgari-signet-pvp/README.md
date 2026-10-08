# RAV Golgari Signet PvP task

Implement the candidate-owned Golgari Signet definition and paid black-green mana ability. In a legal 60-card PvP game, cast and resolve the colorless `{2}` artifact, pay `{1}` and tap it to create one black and one green mana in a non-stack bundle, then spend those exact colors to cast Roofstalker Wight for `{1}{B}`: black pays its colored symbol and green pays its generic symbol. The trace checks the live 2/1 support creature, later-turn combat, a terminal player-zero win, ordered events, and deterministic replay.

The authority changes the mana output to a wrong color pair and requires the live payment trace to reject it. The compile-valid starter stub has the same semantic defect. Only the candidate-owned Golgari Signet module is editable; Roofstalker Wight is a reviewed support card. The task uses the shared Magic workspace filter and does not run a provider trial.
