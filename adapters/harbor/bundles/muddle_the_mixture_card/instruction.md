# Task: implement Muddle the Mixture

Implement the candidate-owned Muddle the Mixture definition in the provided RAV set module. Preserve the Ravnica: City of Guilds printing: collector #60, blue Instant, `{U}{U}`, “Counter target instant or sorcery spell,” and Transmute `{1}{U}{U}`. The printed reminder text says “converted mana cost”; use the current rules meaning of mana value without changing the printed card behavior.

The task authority uses a shuffled legal duel. It checks private Transmute search and reveal, the exact selected card's move to hand and later cast, library shuffle, sorcery timing, an exact Muddle countering Rain of Embers, target spell graveyard movement without resolution, terminal combat, deterministic replay, and source-contract mutations. Keep all implementation in the candidate file. Do not alter task tooling or trusted authority.
