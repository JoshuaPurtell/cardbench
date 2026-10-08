# Ethereal Usher RAV full-game task

Implement Ethereal Usher (#47) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/ethereal_usher.rs`. The RAV printing is a blue `{5}{U}` 2/3 Creature — Spirit with `{U}, {T}: Target creature is unblockable this turn` and Transmute `{1}{U}{U}`.

The authority uses two shuffled legal 60-card decks. It must Transmute a physical Usher at sorcery timing for `{1}{U}{U}`, privately select/reveal another Ethereal Usher with the same mana value, move that object to hand, and shuffle. It then casts an Usher, pays `{U}` and taps it to make the pilot's Watchwolf unblockable, proves the opponent's attempted block is illegal, observes the effect expire, and continues to a normal seat-zero win with deterministic replay.

The source gate pins the normalized source and literal AST before candidate calls and rejects a compile-valid injected side-effect fixture. Gameplay mutations cover the Usher casting cost/body, ability payment/effect, and Transmute presence/cost. The task is diagnostic only: zero cards are credited and rewards remain zero. It does not use provider-backed trials.

Materialize and verify the public source workspace with `python3 varieties/magic/scripts/materialize_ethereal_usher_task.py --workspace PATH --manifest PATH` and the same command with `--verify`.
