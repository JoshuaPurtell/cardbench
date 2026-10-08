//! RED regression for returning an Aura's enchanted creature and all attached
//! Auras to their individual owners' hands as one source-relative instruction.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Effect, Game, GameEvent, ManaCost,
    PlayerId, TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-ATTACHED-GROUP-RETURN-CREATURE";
const MARK: &str = "TST-ATTACHED-GROUP-RETURN-MARK";
const OTHER_AURA: &str = "TST-ATTACHED-GROUP-RETURN-OTHER-AURA";
const ABILITY: &str = "return-enchanted-creature-and-auras";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let is_creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["return-source-attached-creature-and-auras-red"],
        power: is_creature.then_some(2),
        toughness: is_creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

#[test]
fn attached_group_moves_to_each_card_owners_hand_before_the_trigger_finishes() {
    let aura_attachment = || Effect::AttachSourceToTarget {
        target: TargetRequirement::Creature,
        changes: vec![],
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(CREATURE, CardType::Creature, vec![]),
            definition(MARK, CardType::Enchantment, vec![aura_attachment()]),
            definition(OTHER_AURA, CardType::Enchantment, vec![aura_attachment()]),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: MARK,
            ability: TriggeredAbility {
                id: ABILITY,
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::ReturnSourceAttachedPermanentAndAurasToOwnersHands],
            },
        }],
    )
    .expect("game fixture initializes");
    game.register_attachment_bindings([
        AttachmentBinding {
            card_definition: MARK,
            kind: AttachmentKind::Aura,
            target: TargetRequirement::Creature,
            changes: vec![],
            granted_activated_abilities: vec![],
        },
        AttachmentBinding {
            card_definition: OTHER_AURA,
            kind: AttachmentKind::Aura,
            target: TargetRequirement::Creature,
            changes: vec![],
            granted_activated_abilities: vec![],
        },
    ])
    .expect("both Auras register");

    let creature = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("opponent-owned creature enters");
    let mark = game
        .add_card(PlayerId(0), MARK, Zone::Hand)
        .expect("Mark owner setup");
    game.enter_attachment_without_cast(mark, creature)
        .expect("Mark attaches to the creature");
    let other_aura = game
        .add_card(PlayerId(1), OTHER_AURA, Zone::Hand)
        .expect("second Aura owner setup");
    game.enter_attachment_without_cast(other_aura, creature)
        .expect("second Aura attaches to the same creature");

    game.begin_game().expect("P0 upkeep stacks Mark's trigger");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == mark && *ability == ABILITY
    )));
    game.pass_priority(PlayerId(0))
        .expect("Mark controller passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and the return trigger resolves");

    assert_eq!(game.zone_of(creature), Some(Zone::Hand));
    assert_eq!(game.zone_of(mark), Some(Zone::Hand));
    assert_eq!(game.zone_of(other_aura), Some(Zone::Hand));
    let creature_return = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == creature))
        .expect("creature receives an owner-hand event");
    let mark_return = game
        .event_log
        .iter()
        .position(
            |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == mark),
        )
        .expect("Mark receives its owner's hand event");
    let other_aura_return = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == other_aura))
        .expect("other Aura receives its owner's hand event");
    let resolved = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved { source, ability, .. }
                    if *source == mark && *ability == ABILITY
            )
        })
        .expect("trigger has its terminal resolution receipt");
    assert!(creature_return < resolved);
    assert!(mark_return < resolved);
    assert!(other_aura_return < resolved);
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == mark || *card == other_aura
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::StateBasedAction { card, reason }
            if (*card == mark || *card == other_aura)
                && *reason == "Aura is not attached to a battlefield creature"
    )));
    game.validate_invariants()
        .expect("returned attachment group leaves no stale attachment refs");
}
