//! Red regression: every linked delayed-return entrant observes the whole event.
//!
//! CR 603.6a checks all permanents on the battlefield, including newcomers,
//! after one event puts multiple permanents onto it. A linked creature/Aura
//! return is one such event, so the returned Aura must observe both its own
//! entry and the simultaneously returned creature.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, AttachmentBinding,
    AttachmentKind, CardDefinition, CardType, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, PlayerId, Step, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-SIMULTANEOUS-LINKED-RETURN-CREATURE";
const AURA: &str = "TST-SIMULTANEOUS-LINKED-RETURN-AURA";

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
        supported_rules: &["simultaneous-linked-return-entry-observer-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_end_step(game: &mut Game) {
    while game.step != Step::End {
        if game.step == Step::DeclareAttackers {
            game.declare_attackers(game.active_player, &[])
                .expect("empty attackers are declared explicitly");
        }
        pass_pair(game);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The linked return's complete no-priority transition is the regression.
fn linked_delayed_return_checks_its_whole_simultaneous_entry_event() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                AURA,
                CardType::Enchantment,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: vec![],
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: AURA,
            ability: ActivatedAbility {
                id: "exile-linked-group",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ExileAttachedCreatureAndAurasUntilEndStep],
            },
        }],
        [TriggeredAbilityBinding {
            card_definition: AURA,
            ability: TriggeredAbility {
                id: "observe-controlled-nonartifact-entry",
                condition: TriggerCondition::ControlledNonartifactPermanentEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::ReturnAnotherControlledPermanentSharingEnteredCardTypes],
            },
        }],
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }])
    .expect("Aura binding registers");
    let creature = game
        .put_on_battlefield(controller, CREATURE)
        .expect("creature setup");
    let aura = game
        .add_card(controller, AURA, Zone::Hand)
        .expect("Aura setup");
    game.enter_attachment_without_cast(aura, creature)
        .expect("pregame Aura setup attaches without creating a live trigger");
    game.begin_game().expect("game begins");
    let delayed_return_start = game.event_log.len();
    game.activate_ability(
        controller,
        AbilityActivation {
            source: aura,
            ability_id: "exile-linked-group",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("linked exile ability stacks");
    pass_pair(&mut game); // Exile and schedule the delayed return.
    advance_to_end_step(&mut game);

    let order = game
        .view_for_player(controller)
        .expect("controller view is available")
        .pending_decision
        .expect("two simultaneous observer events require an explicit order");
    assert_eq!(order.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(order.trigger_candidates.len(), 2);
    assert!(order.trigger_candidates.iter().all(|entry| {
        entry.source == aura && entry.ability == "observe-controlled-nonartifact-entry"
    }));
    game.submit_decision(
        controller,
        order.id,
        DecisionSelection::TriggerOrder(order.trigger_candidates.clone()),
    )
    .expect("controller orders both simultaneous entry observations");

    let return_observer_triggers = game.event_log[delayed_return_start..]
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked {
                    source,
                    ability: "observe-controlled-nonartifact-entry",
                    ..
                } if *source == aura
            )
        })
        .count();
    eprintln!(
        "linked simultaneous-return trace={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        return_observer_triggers, 2,
        "the returned Aura must observe both entrants from one linked return event"
    );
    game.validate_invariants()
        .expect("linked simultaneous return remains invariant-valid");
}
