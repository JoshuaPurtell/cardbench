//! Regression for Empty the Catacombs' all-player graveyard return.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, PlayerId, Zone,
};
use cardbench_magic_rav::card_definitions;

#[test]
fn empty_the_catacombs_returns_one_creature_card_per_graveyard_to_hand() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-EMPTY-THE-CATACOMBS")
        .expect("Empty the Catacombs definition exists");
    assert!(
        definition
            .supported_rules
            .contains(&"each-player-returns-creature-card-from-graveyard-to-hand")
    );

    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture constructs");
    let spell = game
        .add_card(PlayerId(0), "RAV-EMPTY-THE-CATACOMBS", Zone::Hand)
        .expect("spell setup");
    let first = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("first creature setup");
    let second = game
        .add_card(PlayerId(1), "RAV-BOROS-RECRUIT", Zone::Graveyard)
        .expect("second creature setup");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("spell mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    let first_choice = game
        .view_for_player(PlayerId(0))
        .expect("first player view")
        .pending_decision
        .expect("first graveyard choice is pending");
    assert_eq!(
        first_choice.kind,
        DecisionKind::PublicGraveyardCreatureReturn
    );
    game.submit_decision(
        PlayerId(0),
        first_choice.id,
        DecisionSelection::Objects(vec![first]),
    )
    .expect("first player selects their creature");
    let second_choice = game
        .view_for_player(PlayerId(1))
        .expect("second player view")
        .pending_decision
        .expect("second graveyard choice is pending");
    assert_eq!(
        second_choice.kind,
        DecisionKind::PublicGraveyardCreatureReturn
    );
    game.submit_decision(
        PlayerId(1),
        second_choice.id,
        DecisionSelection::Objects(vec![second]),
    )
    .expect("second player selects their creature");

    assert_eq!(game.zone_of(first), Some(Zone::Hand));
    assert_eq!(game.zone_of(second), Some(Zone::Hand));
    assert_eq!(
        game.canonical_event_log(),
        vec![
            "SpellCast { player: PlayerId(0), card: ObjectId(1) }",
            "ObjectIncarnationAdvanced { object: ObjectId(1), incarnation: 2 }",
            "PriorityPassed { player: PlayerId(0) }",
            "PriorityPassed { player: PlayerId(1) }",
            "DecisionOpened { decision: DecisionId(1), player: PlayerId(0), kind: PublicGraveyardCreatureReturn, visibility: Public, min_selections: 1, max_selections: 1 }",
            "DecisionCompleted { decision: DecisionId(1), player: PlayerId(0), kind: PublicGraveyardCreatureReturn }",
            "DecisionOpened { decision: DecisionId(2), player: PlayerId(1), kind: PublicGraveyardCreatureReturn, visibility: Public, min_selections: 1, max_selections: 1 }",
            "DecisionCompleted { decision: DecisionId(2), player: PlayerId(1), kind: PublicGraveyardCreatureReturn }",
            "CardMoved { card: ObjectId(2), to: Hand }",
            "ObjectIncarnationAdvanced { object: ObjectId(2), incarnation: 2 }",
            "CardMoved { card: ObjectId(3), to: Hand }",
            "ObjectIncarnationAdvanced { object: ObjectId(3), incarnation: 2 }",
            "SpellResolved { card: ObjectId(1) }",
            "CardMoved { card: ObjectId(1), to: Graveyard }",
            "ObjectIncarnationAdvanced { object: ObjectId(1), incarnation: 3 }",
        ]
    );
    game.validate_invariants()
        .expect("all-player graveyard return stays valid");
}
