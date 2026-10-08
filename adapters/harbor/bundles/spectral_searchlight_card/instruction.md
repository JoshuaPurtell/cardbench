Implement Spectral Searchlight (RAV #271) in the candidate-owned module. Keep
the printed colorless `{3}` artifact identity and the `{T}` activation that
targets one player; the target player chooses one colored mana and receives it
when the ability resolves. The full-game authority exercises self-targeting
and opponent-targeting choices, checks exact cast/payment and recipient
provenance, then plays through a terminal result and deterministic replay.
Only the declared module is writable. The local reference grants no card
credit or reward.
