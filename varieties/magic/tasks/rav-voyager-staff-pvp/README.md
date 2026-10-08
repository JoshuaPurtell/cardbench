# Voyager Staff RAV card task

Implement Voyager Staff, RAV #274, as a colorless `{1}` artifact. Its `{2}`, sacrifice-this-artifact activated ability exiles target creature and returns that card to the battlefield under its owner's control at the beginning of the next end step. The candidate module exposes only its definition and activated ability.

The authority uses shuffled legal 60-card decks. Seat zero casts Voyager Staff, activates it at instant timing by paying exactly two generic mana and sacrificing that exact source, and targets the opposing Watchwolf. The trace proves the exact target incarnation is exiled and linked to the delayed action, then returns to its owner's battlefield at the next end step as a new incarnation. Seat zero then casts and attacks with Watchwolf to finish the game. Replay must be deterministic; gameplay mutations cover the activation cost, sacrifice cost, target/effect, and delayed return. Candidate source is pinned and checked before candidate functions are called, including rejection of a compile-valid side-effect injection.

This remains a local diagnostic task. All reward and card-credit values are zero.
