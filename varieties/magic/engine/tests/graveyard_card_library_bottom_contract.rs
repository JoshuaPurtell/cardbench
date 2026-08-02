//! Contract for a target card moving from any public graveyard to owner bottom.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Effect,
    Game, GameEvent, ManaCost, PlayerId, Target, TargetRequirement, Zone,
};

const TROLLER: &str = "TST-GRAVEYARD-BOTTOM-TROLLER";
const TARGET: &str = "TST-GRAVEYARD-BOTTOM-TARGET";
const LIBRARY_CARD: &str = "TST-GRAVEYARD-BOTTOM-LIBRARY";

fn definition(id: &'static str, card_types: BTreeSet<CardType>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["graveyard-card-owner-library-bottom-contract"],
        power: (id == TROLLER).then_some(0),
        toughness: (id == TROLLER).then_some(6),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn activation_can_target_an_opponents_graveyard_card_and_put_it_below_its_library() {
    let mut game = Game::new_with_all_bindings(
        [
            definition(
                TROLLER,
                BTreeSet::from([CardType::Artifact, CardType::Creature]),
            ),
            definition(TARGET, BTreeSet::from([CardType::Instant])),
            definition(LIBRARY_CARD, BTreeSet::from([CardType::Sorcery])),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: TROLLER,
            ability: ActivatedAbility {
                id: "tap-target-graveyard-card-to-owner-library-bottom",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::GraveyardCard],
                effects: vec![Effect::PutTargetGraveyardCardOnOwnersLibraryBottom],
            },
        }],
    )
    .expect("fixture requires generic graveyard-card bottom support");
    let source = game
        .put_on_battlefield(PlayerId(0), TROLLER)
        .expect("source setup");
    game.set_entered_turn_for_setup(source, 0)
        .expect("source ages through pre-game setup");
    let target = game
        .add_card(PlayerId(1), TARGET, Zone::Graveyard)
        .expect("opponent graveyard target setup");
    let existing_bottom = game
        .add_card(PlayerId(1), LIBRARY_CARD, Zone::Library)
        .expect("existing library bottom setup");
    let existing_top = game
        .add_card(PlayerId(1), LIBRARY_CARD, Zone::Library)
        .expect("existing library top setup");
    game.begin_game().expect("game begins");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "tap-target-graveyard-card-to-owner-library-bottom",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("opponent graveyard card is a legal target");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert!(game.object(source).expect("source remains").tapped);
    assert_eq!(game.zone_of(target), Some(Zone::Library));
    assert_eq!(
        game.players[1].library,
        vec![target, existing_bottom, existing_top],
        "the targeted card is below every existing card in its owner's library"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Library } if *card == target
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved {
            source: resolved_source,
            ability: "tap-target-graveyard-card-to-owner-library-bottom",
            ..
        } if *resolved_source == source
    )));
    eprintln!(
        "graveyard_card_library_bottom_trace={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("owner-bottom graveyard transition remains invariant-valid");
}
