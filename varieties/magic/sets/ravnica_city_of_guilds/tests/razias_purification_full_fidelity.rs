//! Full-fidelity contract for Razia's Purification's all-player preservation flow.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, ManaCost,
    PlayerId, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
fn razias_purification_has_exact_all_player_preservation_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-RAZIAS-PURIFICATION")
        .expect("Razia's Purification definition exists");
    assert_eq!(definition.name, "Razia's Purification");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Red, Color::White])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Red, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Sorcery]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"each-player-preserves-up-to-three-controlled-permanents")
    );
}

#[test]
fn razias_purification_collects_all_choices_before_sacrificing_the_rest() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let spell = game
        .add_card(controller, "RAV-RAZIAS-PURIFICATION", Zone::Hand)
        .expect("Purification enters hand");
    let controller_permanents = (0..4)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-WATCHWOLF")
                .expect("controller permanent setup")
        })
        .collect::<Vec<_>>();
    let opponent_permanents = (0..4)
        .map(|_| {
            game.put_on_battlefield(opponent, "RAV-BOROS-RECRUIT")
                .expect("opponent permanent setup")
        })
        .collect::<Vec<_>>();
    game.grant_mana(controller, Color::Red, 1)
        .expect("red payment setup");
    game.grant_mana(controller, Color::White, 1)
        .expect("white payment setup");
    game.grant_mana(controller, Color::Colorless, 4)
        .expect("generic payment setup");

    game.cast_spell(
        controller,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Purification casts");
    pass_pair(&mut game);

    let first_choice = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("controller preserves permanents first");
    assert_eq!(
        first_choice.kind,
        DecisionKind::PreserveControlledPermanents
    );
    assert_eq!(
        (first_choice.min_selections, first_choice.max_selections),
        (0, 3)
    );
    assert_eq!(
        first_choice
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<BTreeSet<_>>(),
        controller_permanents.iter().copied().collect()
    );
    assert_eq!(game.stack.len(), 1, "choice retains resolving spell");
    game.validate_invariants()
        .expect("first public preservation choice validates");

    let controller_preserved = controller_permanents[..3].to_vec();
    game.submit_decision(
        controller,
        first_choice.id,
        DecisionSelection::Objects(controller_preserved.clone()),
    )
    .expect("controller preserves three permanents");

    let second_choice = game
        .view_for_player(opponent)
        .expect("opponent view")
        .pending_decision
        .expect("opponent preserves permanents second");
    assert_eq!(
        second_choice.kind,
        DecisionKind::PreserveControlledPermanents
    );
    assert_eq!(
        (second_choice.min_selections, second_choice.max_selections),
        (0, 3)
    );
    assert_eq!(
        second_choice
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<BTreeSet<_>>(),
        opponent_permanents.iter().copied().collect()
    );
    assert!(
        controller_permanents
            .iter()
            .chain(&opponent_permanents)
            .all(|permanent| game.zone_of(*permanent) == Some(Zone::Battlefield)),
        "no permanent moves before every player has selected"
    );
    game.validate_invariants()
        .expect("second public preservation choice validates");

    let opponent_preserved = Vec::new();
    game.submit_decision(
        opponent,
        second_choice.id,
        DecisionSelection::Objects(opponent_preserved.clone()),
    )
    .expect("opponent may preserve no permanents");

    for permanent in controller_preserved.iter().chain(&opponent_preserved) {
        assert_eq!(game.zone_of(*permanent), Some(Zone::Battlefield));
    }
    let controller_sacrificed = controller_permanents[3];
    let opponent_sacrificed = opponent_permanents[0];
    assert_eq!(game.zone_of(controller_sacrificed), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(opponent_sacrificed), Some(Zone::Graveyard));
    assert!(
        opponent_permanents
            .iter()
            .all(|permanent| { game.zone_of(*permanent) == Some(Zone::Graveyard) })
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
    let sacrifice_positions = game
        .event_log
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::SacrificedByEffect {
                source,
                player,
                permanent,
            } if *source == spell
                && (player == &controller && permanent == &controller_sacrificed
                    || player == &opponent && opponent_permanents.contains(permanent)) =>
            {
                Some(index)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let terminal = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        .expect("spell terminal receipt exists");
    assert_eq!(sacrifice_positions.len(), 5);
    assert!(
        sacrifice_positions
            .into_iter()
            .all(|position| position < terminal)
    );
    game.validate_invariants()
        .expect("all-player preservation resolution preserves invariants");
}
