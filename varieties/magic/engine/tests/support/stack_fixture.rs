//! Shared setup for fabricated stack-state invariant probes.
//!
//! `StackObjectId` is intentionally allocated by the game state machine. A
//! direct fixture can model a shape-valid corrupted stack only after it has
//! advanced the private monotonic allocator through an ordinary spell cast.

use cardbench_magic_engine::{CastRequest, Game, PlayerId, Zone};

pub fn advance_stack_identity(game: &mut Game, definition: &'static str) {
    let card = game
        .add_card(PlayerId(0), definition, Zone::Hand)
        .expect("stack-identity warmup card enters hand");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("ordinary warmup spell allocates a stack identity");
    for player in (0..game.players.len()).map(PlayerId) {
        game.pass_priority(player)
            .expect("ordinary warmup spell resolves through every priority pass");
    }
    game.clear_event_log();
}
