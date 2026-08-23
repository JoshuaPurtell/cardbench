//! Full-fidelity regression for Life from the Loam's land-card recursion.

use cardbench_magic_engine::{
    CardType, Color, DecisionSelection, Game, Keyword, ManaCost, PlayerId, PolicyAction, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn life_from_the_loam_has_dredge_and_policy_submitted_land_recursion() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LIFE-FROM-THE-LOAM")
        .expect("Life from the Loam definition exists");
    assert_eq!(definition.name, "Life from the Loam");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Green, Color::Green])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Sorcery].into_iter().collect()
    );
    assert!(definition.keywords.contains(&Keyword::Dredge(3)));
    assert!(
        definition
            .supported_rules
            .contains(&"return-up-to-three-land-cards-from-graveyard")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"policy-submitted-public-graveyard-land-selection")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}

#[test]
fn life_from_the_loam_returns_only_its_controllers_land_cards_to_hand() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let loam = game
        .add_card(PlayerId(0), "RAV-LIFE-FROM-THE-LOAM", Zone::Hand)
        .expect("Life from the Loam setup");
    let own_lands = [
        game.add_card(PlayerId(0), "RAV-FOREST", Zone::Graveyard)
            .expect("Forest setup"),
        game.add_card(PlayerId(0), "RAV-MOUNTAIN", Zone::Graveyard)
            .expect("Mountain setup"),
        game.add_card(PlayerId(0), "RAV-ISLAND", Zone::Graveyard)
            .expect("Island setup"),
    ];
    let opponents_land = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Graveyard)
        .expect("opponent land setup");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("mana setup");

    game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: loam,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Life from the Loam casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("land recursion opens a public controller choice");
    game.submit_policy_move(
        PlayerId(0),
        "life-from-the-loam-full-fidelity-test.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(own_lands.to_vec()),
        },
    )
    .expect("controller returns all three legal lands");

    assert!(
        own_lands
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Hand))
    );
    assert_eq!(game.zone_of(opponents_land), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(loam), Some(Zone::Graveyard));
    println!("Life from the Loam trace: {:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("land recursion preserves zone and stack invariants");
}
