//! Red regression: no-cast Aura entry must observe its ordinary ETB trigger.
//!
//! `enter_attachment_without_cast` is a bounded fixture/rules primitive, but
//! entering the battlefield is still an entry event. The trigger must retain
//! the Aura's new battlefield incarnation and be placed only after its
//! attachment transition has reached the ordinary SBA boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Effect, Game, GameEvent, ManaCost,
    PlayerId, TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-NO-CAST-AURA-ENTRY-CREATURE";
const AURA: &str = "TST-NO-CAST-AURA-ENTRY-AURA";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["no-cast-aura-entry-etb-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

#[test]
fn no_cast_aura_entry_queues_its_etb_after_attachment_and_sba() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                AURA,
                CardType::Enchantment,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: vec![],
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: AURA,
            ability: TriggeredAbility {
                id: "aura-entry-life",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::ControlledCreature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }])
    .expect("Aura binding registers");
    let creature = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("creature setup");
    let aura = game
        .add_card(PlayerId(0), AURA, Zone::Hand)
        .expect("Aura setup");
    game.begin_game().expect("game begins");
    game.clear_event_log();

    game.enter_attachment_without_cast(aura, creature)
        .expect("legal no-cast Aura entry");
    eprintln!("no-cast Aura entry trace={:?}", game.canonical_event_log());

    let aura_incarnation = game.object(aura).expect("Aura remains live").incarnation;
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability: "aura-entry-life",
                ..
            } if *source == aura && *source_incarnation == aura_incarnation
        )),
        "no-cast entry must preserve the Aura ETB observation after attachment/SBA"
    );
    game.validate_invariants()
        .expect("queued Aura ETB leaves valid state");
}
