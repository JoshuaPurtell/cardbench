//! Red regression for Empty the Catacombs' required public graveyard choices.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, PlayerId, Zone,
};
use cardbench_magic_rav::card_definitions;

#[test]
fn empty_the_catacombs_waits_for_each_players_public_graveyard_choice() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture constructs");
    let spell = game
        .add_card(PlayerId(0), "RAV-EMPTY-THE-CATACOMBS", Zone::Hand)
        .expect("spell setup");
    let first = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("first creature setup");
    let selected = game
        .add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Graveyard)
        .expect("selected creature setup");
    let opponent_creature = game
        .add_card(PlayerId(1), "RAV-BOROS-RECRUIT", Zone::Graveyard)
        .expect("opponent creature setup");
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

    // Each player chooses a creature card in their own public graveyard as
    // the spell resolves. Resolution must pause at that policy boundary; it
    // cannot silently use insertion order and move a card before the owner
    // can choose it.
    let controller_choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("Empty the Catacombs must open a public graveyard-choice decision");
    assert_eq!(
        controller_choice.kind,
        DecisionKind::PublicGraveyardCreatureReturn
    );
    assert_eq!(controller_choice.min_selections, 1);
    assert_eq!(controller_choice.max_selections, 1);
    assert_eq!(
        controller_choice
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![first, selected]
    );
    assert_eq!(game.zone_of(first), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(selected), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(opponent_creature), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("a pending public-zone choice is an invariant-valid state");

    game.submit_decision(
        PlayerId(0),
        controller_choice.id,
        DecisionSelection::Objects(vec![selected]),
    )
    .expect("controller selects the second creature, not the insertion-order first");
    let opponent_choice = game
        .view_for_player(PlayerId(1))
        .expect("opponent view")
        .pending_decision
        .expect("opponent public graveyard choice is pending");
    assert_eq!(
        opponent_choice.kind,
        DecisionKind::PublicGraveyardCreatureReturn
    );
    assert_eq!(
        opponent_choice
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![opponent_creature]
    );
    game.submit_decision(
        PlayerId(1),
        opponent_choice.id,
        DecisionSelection::Objects(vec![opponent_creature]),
    )
    .expect("opponent selects their creature");

    assert_eq!(game.zone_of(first), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(selected), Some(Zone::Hand));
    assert_eq!(game.zone_of(opponent_creature), Some(Zone::Hand));
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
            "CardMoved { card: ObjectId(3), to: Hand }",
            "ObjectIncarnationAdvanced { object: ObjectId(3), incarnation: 2 }",
            "CardMoved { card: ObjectId(4), to: Hand }",
            "ObjectIncarnationAdvanced { object: ObjectId(4), incarnation: 2 }",
            "SpellResolved { card: ObjectId(1) }",
            "CardMoved { card: ObjectId(1), to: Graveyard }",
            "ObjectIncarnationAdvanced { object: ObjectId(1), incarnation: 3 }",
        ]
    );
    game.validate_invariants()
        .expect("the selected public graveyard returns stay valid");
}
