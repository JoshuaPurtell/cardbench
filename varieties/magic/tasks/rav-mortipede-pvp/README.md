# Mortipede RAV full-game task

Implement Mortipede (#96) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/mortipede.rs`. Keep the printed definition and activated ability in this candidate-owned module. It is a black `{3}{B}` 4/1 creature. Its `{2}{G}` ability gives itself “must be blocked if able” until end of turn.

The deterministic shuffled 60-card duel casts Mortipede, pays the exact green activation, resolves it before declaring attackers, and verifies the opponent assigns an available Lurking Informant as a blocker. The two creatures exchange combat damage and die. The remaining legal game continues until seat zero wins with Watchwolves. The authority verifies source ownership, actual mana receipts, required-block assignment, full-game terminal state, deterministic replay, and semantic mutations to the ability effect/cost and creature characteristics.

This is an uncredited local diagnostic. It does not run a provider or grant card credit, set completion, or Harbor reward.
