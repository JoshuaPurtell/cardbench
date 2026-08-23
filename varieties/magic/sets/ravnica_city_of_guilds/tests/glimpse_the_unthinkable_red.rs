//! Red discovery contract for Glimpse the Unthinkable's absent typed mill slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Step, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn glimpse_requires_its_exact_target_player_mill_definition() {
    let glimpse = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GLIMPSE-THE-UNTHINKABLE")
        .expect("Glimpse the Unthinkable definition exists");

    assert_eq!(glimpse.name, "Glimpse the Unthinkable");
    assert_eq!(
        glimpse.mana_cost,
        ManaCost::with_colors(0, [Color::Blue, Color::Black])
    );
    assert_eq!(glimpse.colors, BTreeSet::from([Color::Blue, Color::Black]));
    assert_eq!(glimpse.card_types, BTreeSet::from([CardType::Sorcery]));
    assert_eq!(glimpse.effects, [Effect::MillTargetPlayer { count: 10 }]);
    assert_eq!(
        glimpse.effects[0].target_requirement(),
        Some(TargetRequirement::Player)
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&glimpse.id));
    assert!(glimpse.supported_rules.contains(&"targeted-mill-ten"));
}

#[test]
fn glimpse_catalog_mapping_names_only_the_typed_full_definition() {
    assert_eq!(
        executable_definition_id_for_collector(208),
        Ok("RAV-GLIMPSE-THE-UNTHINKABLE")
    );
    assert!(
        card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-GLIMPSE-THE-UNTHINKABLE"),
        "the catalog maps only to the complete typed definition"
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn glimpse_mills_exactly_ten_selected_player_library_cards() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let mut opponent_library = BTreeSet::new();
    for player in [PlayerId(0), PlayerId(1)] {
        for _ in 0..10 {
            let card = game
                .add_card(player, "RAV-PLAINS", Zone::Library)
                .expect("library fixture card exists");
            if player == PlayerId(1) {
                opponent_library.insert(card);
            }
        }
    }
    let glimpse = game
        .add_card(PlayerId(0), "RAV-GLIMPSE-THE-UNTHINKABLE", Zone::Hand)
        .expect("Glimpse begins in hand");
    game.begin_game().expect("fixture starts game");
    while game.step != Step::PrecombatMain {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public state")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
        } else {
            pass_pair(&mut game);
        }
    }
    game.add_mana_from_action(PlayerId(0), Color::Blue, 1)
        .expect("blue mana payment is a legal action");
    game.add_mana_from_action(PlayerId(0), Color::Black, 1)
        .expect("black mana payment is a legal action");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: glimpse,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Glimpse accepts one player target");
    pass_pair(&mut game);

    println!(
        "Glimpse the Unthinkable trace: {:#?}",
        game.canonical_event_log()
    );
    assert!(game.players[1].library.is_empty());
    assert_eq!(game.players[1].graveyard.len(), 10);
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::CardMoved { card, to: Zone::Graveyard }
                    if opponent_library.contains(card)
            ))
            .count(),
        10
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellResolved { card } if *card == glimpse
    )));
    game.validate_invariants()
        .expect("targeted mill preserves invariant state");
}
