//! Red regression: removing a player's permanent source directly as that
//! player leaves must not retain an impossible linked-hand-exile group.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target, TargetRequirement, Zone,
};

const SOURCE: &str = "DEPARTED-HAND-EXILE-SOURCE";
const EXILED_CARD: &str = "DEPARTED-HAND-EXILE-MEMBER";
const KILLER: &str = "DEPARTED-HAND-EXILE-KILLER";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["departed-linked-hand-exile-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The full multi-player ownership transition is the regression.
fn departing_owner_expires_its_linked_hand_exile_group() {
    let survivor = PlayerId(0);
    let bystander = PlayerId(1);
    let departing_source_owner = PlayerId(2);
    let mut game = Game::new_with_all_bindings(
        [
            definition(SOURCE, BTreeSet::from([CardType::Artifact]), vec![]),
            definition(EXILED_CARD, BTreeSet::from([CardType::Artifact]), vec![]),
            definition(
                KILLER,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: "exile-controller-hand",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ExileControllerHandLinkedToSource],
            },
        }],
    )
    .expect("three-player fixture initializes");
    let source = game
        .put_on_battlefield(departing_source_owner, SOURCE)
        .expect("departing owner starts with the source");
    let exiled_card = game
        .add_card(departing_source_owner, EXILED_CARD, Zone::Hand)
        .expect("departing owner starts with a hand card");
    let killer = game
        .add_card(survivor, KILLER, Zone::Hand)
        .expect("survivor starts with lethal damage");

    game.pass_priority(survivor)
        .expect("survivor passes to the bystander");
    game.pass_priority(bystander)
        .expect("bystander passes to the source controller");
    game.activate_ability(
        departing_source_owner,
        AbilityActivation {
            source,
            ability_id: "exile-controller-hand",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("source controller activates the linked hand-exile ability");
    game.pass_priority(departing_source_owner)
        .expect("source controller passes");
    game.pass_priority(survivor).expect("survivor passes");
    game.pass_priority(bystander)
        .expect("bystander resolves the linked hand exile");
    assert_eq!(game.zone_of(exiled_card), Some(Zone::Exile));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::HandExiledWithSource { source: recorded, cards, .. }
            if *recorded == source && cards == &vec![exiled_card])
    }));

    game.cast_spell(
        survivor,
        CastRequest {
            card: killer,
            targets: vec![Target::Player(departing_source_owner)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("survivor targets the source owner with lethal damage");
    game.pass_priority(survivor).expect("survivor passes");
    game.pass_priority(bystander).expect("bystander passes");
    game.pass_priority(departing_source_owner)
        .expect("owner departure expires the linked hand-exile group");

    assert!(game.players[departing_source_owner.0].lost);
    assert!(game.object(source).is_err());
    assert!(game.object(exiled_card).is_err());
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::LinkedHandExileExpired { source: recorded, cards, .. }
            if *recorded == source && cards == &vec![exiled_card])
    }));
    game.validate_invariants()
        .expect("no linked hand-exile provenance survives its owner's departure");
}
