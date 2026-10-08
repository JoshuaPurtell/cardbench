# Vedalken Entrancer full-game card task

Implement the candidate-owned Vedalken Entrancer module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vedalken_entrancer.rs`. Vedalken Entrancer is RAV collector #74: a blue 1/4 Creature with Defender that costs `{3}{U}`. Its activated ability is `{U}, {T}: Target player mills two cards.` Keep the ability identifier `tap-blue-mill-two`.

The authority runs a shuffled 60-card PvP duel. It casts the exact Entrancer object, waits until that object can legally pay a tap-symbol cost, then activates on the opponent's turn. The trace checks the blue ability payment, source tap, exact player target, and the identity and owner of the two top library objects moved to the opponent's graveyard. A Watchwolf continues through combat until the pilot wins. The duel is replayed deterministically, and five gameplay mutations check the mill amount, tap cost, blue cost, player target, and Defender.

This is a diagnostic pilot only. The report covers one identity among 291 RAV names and 306 printings and awards zero card credit and zero reward. Do not run a provider trial for reference validation. Materialize with `materialize_vedalken_entrancer_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
