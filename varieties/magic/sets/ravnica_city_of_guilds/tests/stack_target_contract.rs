//! Adversarial public RAV probes for stack ordering and target revalidation.
//!
//! These tests use only executable RAV card definitions. They deliberately
//! assert the replay events around a response, a target that becomes illegal,
//! and the distinction between a resolving counterspell and the rules-based
//! all-targets-illegal counter.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone};
use cardbench_magic_rav::card_definitions;

fn rav_game() -> Game {
    Game::new(card_definitions(), 2).expect("RAV stack probe game initializes")
}

fn cast(game: &mut Game, player: PlayerId, card: u64, target: Target) {
    game.cast_spell(
        player,
        CastRequest {
            card: cardbench_magic_engine::ObjectId(card),
            targets: vec![target],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("prepared RAV spell is legal");
    game.validate_invariants()
        .expect("cast preserves stack and priority invariants");
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first)
        .expect("first player passes priority");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player passes and resolves the top spell");
    game.validate_invariants()
        .expect("resolution preserves stack and priority invariants");
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> usize {
    events
        .iter()
        .position(predicate)
        .expect("required canonical event was emitted")
}

#[test]
fn response_resolves_lifo_before_the_original_spell() {
    let mut game = rav_game();
    let char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("Char enters player zero hand");
    let helix = game
        .add_card(PlayerId(1), "RAV-LIGHTNING-HELIX", Zone::Hand)
        .expect("Helix enters player one hand");
    game.grant_mana(PlayerId(0), Color::Red, 3)
        .expect("Char mana");
    game.grant_mana(PlayerId(1), Color::White, 1)
        .expect("Helix white mana");
    game.grant_mana(PlayerId(1), Color::Red, 1)
        .expect("Helix red mana");
    game.clear_event_log();

    cast(&mut game, PlayerId(0), char.0, Target::Player(PlayerId(1)));
    game.pass_priority(PlayerId(0))
        .expect("the caster passes before the response");
    cast(&mut game, PlayerId(1), helix.0, Target::Player(PlayerId(0)));
    resolve_top(&mut game);
    assert_eq!(
        game.stack.len(),
        1,
        "the original spell remains on the stack"
    );
    assert_eq!(
        game.priority,
        PlayerId(0),
        "active player receives priority"
    );
    resolve_top(&mut game);

    assert!(game.stack.is_empty());
    assert_eq!(game.players[0].life, 15);
    assert_eq!(game.players[1].life, 19);
    let helix_resolution = event_index(
        &game.event_log,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == helix),
    );
    let char_resolution = event_index(
        &game.event_log,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == char),
    );
    assert!(
        helix_resolution < char_resolution,
        "the response must resolve before the spell below it"
    );
    game.validate_invariants()
        .expect("completed LIFO response trace remains valid");
}

#[test]
fn target_that_leaves_battlefield_counters_the_pending_spell_by_rules() {
    let mut game = rav_game();
    let gasp = game
        .add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)
        .expect("Last Gasp enters player zero hand");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("Char enters player one hand");
    let victim = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("Brownscale is a targetable creature");
    game.grant_mana(PlayerId(0), Color::Black, 1)
        .expect("Last Gasp mana");
    game.grant_mana(PlayerId(1), Color::Red, 3)
        .expect("Char mana");
    game.clear_event_log();

    cast(&mut game, PlayerId(0), gasp.0, Target::Permanent(victim));
    game.pass_priority(PlayerId(0))
        .expect("the caster passes before the response");
    cast(&mut game, PlayerId(1), char.0, Target::Permanent(victim));
    resolve_top(&mut game);

    assert_eq!(game.zone_of(victim), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(char), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 1, "Last Gasp remains pending");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(gasp), Some(Zone::Graveyard));
    assert_eq!(game.players[0].life, 20);
    assert_eq!(
        game.players[1].life, 18,
        "Char still damages its controller"
    );
    let char_resolution = event_index(
        &game.event_log,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == char),
    );
    let rules_counter = event_index(
        &game.event_log,
        |event| matches!(event, GameEvent::SpellCounteredByRules { card } if *card == gasp),
    );
    assert!(char_resolution < rules_counter);
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == gasp)),
        "a spell with no legal targets cannot emit SpellResolved"
    );
    game.validate_invariants()
        .expect("rules-counter trace remains valid");
}
