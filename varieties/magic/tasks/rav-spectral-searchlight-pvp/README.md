# RAV Spectral Searchlight single-card PvP task

This task qualifies Spectral Searchlight (RAV #271) in a shuffled, full 60-card
two-player duel. The pilot casts the colorless artifact for exactly `{3}`, then
uses two distinct tapped sources to target both players. Each target player
makes their own colored-mana choice: the pilot chooses white and spends that
mana, together with one Plains activation, to cast Courier Hawk; the opponent
chooses blue and receives that mana in their pool. The authority also verifies
that the artifact controller cannot submit the opponent's choice.

Both activations must remain on the stack through priority, open and complete a
recipient-owned mana-color decision, add exactly one chosen mana to the target,
and resolve. The match reaches a terminal pilot win, replays deterministically,
and detects mutations to the spell cost, tap cost, target requirement, and
target-player mana effect. The scorer reports diagnostic evidence only: card
credit, pilot reward, and Harbor reward remain zero.

Materialize with `materialize_spectral_searchlight_task.py --workspace PATH
--manifest PATH`; add `--verify` to check the materialized workspace.
