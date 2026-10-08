# Autochthon Wurm full-game card task

Implement the candidate-owned Autochthon Wurm definition at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/autochthon_wurm.rs`. Autochthon Wurm is RAV collector #191, a green-white 9/14 Creature costing `{10}{G}{G}{G}{W}{W}` with Convoke and Trample.

The reference authority runs a shuffled 60-card RAV duel from normal opening hands. The pilot develops a Selesnya board from cards in its library and casts Autochthon Wurm by tapping nine distinct creatures for Convoke: two contribute green, two contribute white, and five contribute generic mana; six mana is also paid from the pool. The opponent casts Birds of Paradise and Caregiver. Both block Autochthon Wurm. The attacking player submits a public order for those exact blockers; the Wurm deals lethal damage to each and seven excess Trample damage to the opponent. The full duel continues to a terminal pilot win. The authority checks typed event order, post-combat zones, deterministic replay, and gameplay mutations.

This is a diagnostic pilot only. It reports one identity among 291 RAV names and 306 printings and awards zero card credit, zero pilot reward, and zero Harbor reward. No provider trial is part of the reference checks. Materialize with `materialize_autochthon_wurm_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
