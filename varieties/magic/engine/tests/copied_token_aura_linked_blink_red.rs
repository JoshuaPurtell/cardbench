//! RED: an Aura-linked blink of a copied creature token must not retain a
//! non-existent primary member.
//!
//! Flickerform-style text exiles the enchanted token and every Aura attached
//! to it.  The token immediately ceases to exist, so the contingent return
//! cannot return the attached Auras either.  This exercises that rule through
//! a real copied-token creation trigger, rather than fabricating a token in a
//! live game.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, AttachmentBinding,
    AttachmentKind, CardDefinition, CardType, ContinuousChange, Effect, Game, GameEvent, ManaCost,
    PlayerId, TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-COPIED-TOKEN-LINKED-BLINK-CREATURE";
const COPY_AURA: &str = "TST-COPIED-TOKEN-LINKED-BLINK-COPY-AURA";
const BLINK_AURA: &str = "TST-COPIED-TOKEN-LINKED-BLINK-AURA";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["copied-token-aura-linked-blink-red"],
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

#[test]
#[allow(clippy::too_many_lines)] // The copied-token/Aura departure transcript is the contract.
fn linked_blink_of_a_copied_token_exiles_auras_without_an_impossible_return_group() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                COPY_AURA,
                BTreeSet::from([CardType::Enchantment]),
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: vec![],
                }],
            ),
            definition(
                BLINK_AURA,
                BTreeSet::from([CardType::Enchantment]),
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: vec![ContinuousChange::ModifyPowerToughness {
                        power: 1,
                        toughness: 1,
                    }],
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: BLINK_AURA,
            ability: ActivatedAbility {
                id: "exile-linked-copied-token",
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
            card_definition: COPY_AURA,
            ability: TriggeredAbility {
                id: "copy-attached-creature-at-upkeep",
                condition: TriggerCondition::BeginningOfAttachedCreaturesControllerUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::CreateTokenCopyOfAttachedCreature],
            },
        }],
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([
        AttachmentBinding {
            card_definition: COPY_AURA,
            kind: AttachmentKind::Aura,
            target: TargetRequirement::ControlledCreature,
            changes: vec![],
            granted_activated_abilities: vec![],
        },
        AttachmentBinding {
            card_definition: BLINK_AURA,
            kind: AttachmentKind::Aura,
            target: TargetRequirement::ControlledCreature,
            changes: vec![ContinuousChange::ModifyPowerToughness {
                power: 1,
                toughness: 1,
            }],
            granted_activated_abilities: vec![],
        },
    ])
    .expect("Aura bindings register");

    let creature = game
        .put_on_battlefield(controller, CREATURE)
        .expect("creature setup");
    let copy_aura = game
        .add_card(controller, COPY_AURA, Zone::Hand)
        .expect("copy Aura setup");
    let blink_aura = game
        .add_card(controller, BLINK_AURA, Zone::Hand)
        .expect("linked blink Aura setup");
    game.enter_attachment_without_cast(copy_aura, creature)
        .expect("pregame copy Aura setup attaches");
    game.begin_game()
        .expect("game begins at the controller upkeep");
    pass_pair(&mut game);
    let token = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated { player, token } if *player == controller => Some(*token),
            _ => None,
        })
        .expect("upkeep ability creates one copied token");
    game.enter_attachment_without_cast(blink_aura, token)
        .expect("linked blink Aura may enter attached to the copied token");

    game.clear_event_log();
    game.activate_ability(
        controller,
        AbilityActivation {
            source: blink_aura,
            ability_id: "exile-linked-copied-token",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("linked blink activation stacks");
    pass_pair(&mut game);

    eprintln!(
        "copied-token Aura-linked blink trace={:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.object(token),
        Err(cardbench_magic_engine::RulesError::UnknownCard(token)),
        "the copied token must cease after its battlefield departure"
    );
    assert_eq!(game.zone_of(blink_aura), Some(Zone::Exile));
    assert!(game.event_log.iter().any(
        |event| matches!(event, GameEvent::TokenCeasedToExist { token: ceased } if *ceased == token),
    ));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DelayedActionScheduled { .. } | GameEvent::DelayedActionConsumed { .. }
        )),
        "the failed primary return makes the linked Aura return contingent false"
    );
    game.validate_invariants()
        .expect("token cessation and Aura exile remain auditable");
}
