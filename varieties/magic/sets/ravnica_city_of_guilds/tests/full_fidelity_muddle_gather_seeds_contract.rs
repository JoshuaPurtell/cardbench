//! Public RAV contracts for one explicit compatibility slice and two complete spells.
//!
//! These tests intentionally use semantic card identifiers and executable
//! effects only; they do not reproduce card rules text.

use cardbench_magic_engine::{
    CastRequest, Color, ConvokeContribution, ConvokePayment, Game, GameEvent, PlayerId, Target,
    Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn game() -> Game {
    Game::new(card_definitions(), 2).expect("RAV full-fidelity game initializes")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("priority holder passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second pass resolves the top stack object");
    game.validate_invariants()
        .expect("resolution preserves the game-state invariants");
}

#[test]
#[allow(clippy::too_many_lines)] // Counterspell and transmute fidelity checks share one transcript.
fn muddle_exercises_its_explicit_counterspell_and_transmute_compatibility_slices() {
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-MUDDLE-THE-MIXTURE"),
        "Muddle remains out of the full-fidelity manifest until activated abilities use the stack"
    );

    let mut counter_game = game();
    let char = counter_game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("target spell enters hand");
    let muddle = counter_game
        .add_card(PlayerId(1), "RAV-MUDDLE-THE-MIXTURE", Zone::Hand)
        .expect("counterspell enters hand");
    counter_game
        .grant_mana(PlayerId(0), Color::Red, 3)
        .expect("target spell mana");
    counter_game
        .grant_mana(PlayerId(1), Color::Blue, 2)
        .expect("counterspell mana");
    counter_game.clear_event_log();

    counter_game
        .cast_spell(
            PlayerId(0),
            CastRequest {
                card: char,
                targets: vec![Target::Player(PlayerId(1))],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect("target spell casts");
    counter_game
        .pass_priority(PlayerId(0))
        .expect("caster offers response window");
    counter_game
        .cast_spell(
            PlayerId(1),
            CastRequest {
                card: muddle,
                targets: vec![Target::Spell(char)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect("Muddle legally targets an instant spell on the stack");
    resolve_top(&mut counter_game);

    assert_eq!(counter_game.zone_of(char), Some(Zone::Graveyard));
    assert_eq!(counter_game.zone_of(muddle), Some(Zone::Graveyard));
    assert!(counter_game.event_log.iter().any(|event| {
        matches!(event, GameEvent::SpellCountered { card, source } if *card == char && *source == muddle)
    }));
    assert!(
        !counter_game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == char)),
        "the countered spell must not resolve"
    );
    counter_game
        .validate_invariants()
        .expect("counterspell trace preserves invariants");

    let mut transmute_game = game();
    let muddle = transmute_game
        .add_card(PlayerId(0), "RAV-MUDDLE-THE-MIXTURE", Zone::Hand)
        .expect("Muddle enters hand");
    let found = transmute_game
        .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Library)
        .expect("same-value card enters library");
    transmute_game
        .grant_mana(PlayerId(0), Color::Blue, 3)
        .expect("transmute cost is available");
    transmute_game.clear_event_log();

    transmute_game
        .transmute(PlayerId(0), muddle, Some(found))
        .expect("Muddle transmute searches, reveals, and shuffles");

    assert_eq!(transmute_game.zone_of(muddle), Some(Zone::Graveyard));
    assert_eq!(transmute_game.zone_of(found), Some(Zone::Hand));
    assert!(transmute_game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::CardRevealed {
                player: PlayerId(0),
                card,
                definition: "RAV-LIGHTNING-HELIX",
            } if *card == found
        )
    }));
    assert!(transmute_game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::Transmuted {
                player: PlayerId(0),
                discarded,
                found: Some(selected),
            } if *discarded == muddle && *selected == found
        )
    }));
    transmute_game
        .validate_invariants()
        .expect("transmute trace preserves invariants");
}

#[test]
fn gather_courage_uses_colored_convoke_and_applies_its_temporary_modifier() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-GATHER-COURAGE"));
    let mut game = game();
    let gather = game
        .add_card(PlayerId(0), "RAV-GATHER-COURAGE", Zone::Hand)
        .expect("Gather Courage enters hand");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
        .expect("green creature is available for convoke and targeting");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: gather,
            targets: vec![Target::Permanent(target)],
            convoke: vec![ConvokePayment {
                creature: target,
                contribution: ConvokeContribution::Color(Color::Green),
            }],
            payment_mana_abilities: vec![],
        },
    )
    .expect("a green creature pays the green convoke requirement");
    resolve_top(&mut game);

    let characteristics = game
        .characteristics(target)
        .expect("modified creature remains on the battlefield");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(4), Some(5))
    );
    assert!(game.object(target).expect("target exists").tapped);
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution: Some(Color::Green),
            } if *creature == target
        )
    }));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::ContinuousEffectCreated { source, target: effect_target, .. }
            if *source == gather && *effect_target == target)
    }));
    game.validate_invariants()
        .expect("convoke modifier trace preserves invariants");
}

#[test]
fn seeds_tracks_three_target_slots_and_resolves_the_legal_slots_independently() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-SEEDS-OF-STRENGTH"));
    let mut game = game();
    let seeds = game
        .add_card(PlayerId(0), "RAV-SEEDS-OF-STRENGTH", Zone::Hand)
        .expect("Seeds of Strength enters hand");
    let victim = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("response target is available");
    let survivor = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("two target slots can share this creature");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("response spell enters hand");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("Seeds green mana");
    game.grant_mana(PlayerId(0), Color::White, 1)
        .expect("Seeds white mana");
    game.grant_mana(PlayerId(1), Color::Red, 3)
        .expect("response spell mana");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: seeds,
            targets: vec![
                Target::Permanent(victim),
                Target::Permanent(survivor),
                Target::Permanent(survivor),
            ],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("each printed target slot is supplied, including a repeated target");
    game.pass_priority(PlayerId(0))
        .expect("caster offers response window");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(victim)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("response removes one Seeds target");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(victim), Some(Zone::Graveyard));

    resolve_top(&mut game);
    let characteristics = game
        .characteristics(survivor)
        .expect("the still-legal target remains on the battlefield");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(4), Some(5)),
        "two legal target slots each apply their independent modifier"
    );
    let survivor_effects = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(event, GameEvent::ContinuousEffectCreated { source, target, .. }
                if *source == seeds && *target == survivor)
        })
        .count();
    assert_eq!(survivor_effects, 2);
    assert!(
        game.event_log
            .iter()
            .any(|event| { matches!(event, GameEvent::SpellResolved { card } if *card == seeds) })
    );
    game.validate_invariants()
        .expect("partial target resolution preserves invariants");
}
