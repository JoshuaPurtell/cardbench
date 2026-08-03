//! Red regression: a copied creature-land token must create a land-entry event.
//!
//! Token-copy creation already captures ordinary ETBs using the copied card
//! definition.  A copied creature-land must additionally reach the distinct
//! `LandEntersBattlefield` pipeline, just as a physical creature-land entry
//! does.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, DecisionKind, DecisionSelection,
    Effect, Game, GameEvent, ManaCost, PlayerId, TargetRequirement, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const CREATURE_LAND: &str = "TST-COPIED-CREATURE-LAND";
const COPY_AURA: &str = "TST-COPIED-CREATURE-LAND-AURA";

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
        supported_rules: &["copied-creature-land-token-entry-red"],
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
#[allow(clippy::too_many_lines)] // The full upkeep/copy/entry trigger transcript is the regression contract.
fn copied_creature_land_token_captures_its_land_entry_trigger() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                CREATURE_LAND,
                BTreeSet::from([CardType::Creature, CardType::Land]),
                vec![],
            ),
            definition(
                COPY_AURA,
                BTreeSet::from([CardType::Enchantment]),
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
        [
            TriggeredAbilityBinding {
                card_definition: COPY_AURA,
                ability: TriggeredAbility {
                    id: "copy-attached-creature-at-upkeep",
                    condition: TriggerCondition::BeginningOfAttachedCreaturesControllerUpkeep,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::CreateTokenCopyOfAttachedCreature],
                },
            },
            TriggeredAbilityBinding {
                card_definition: CREATURE_LAND,
                ability: TriggeredAbility {
                    id: "copied-creature-land-entry",
                    condition: TriggerCondition::LandEntersBattlefield,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::GainLifeController { amount: 1 }],
                },
            },
        ],
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: COPY_AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::ControlledCreature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }])
    .expect("copy Aura binding registers");
    let creature_land = game
        .put_on_battlefield(controller, CREATURE_LAND)
        .expect("creature-land setup");
    let aura = game
        .add_card(controller, COPY_AURA, Zone::Hand)
        .expect("Aura setup");
    game.enter_attachment_without_cast(aura, creature_land)
        .expect("pregame Aura setup attaches");
    game.begin_game()
        .expect("game begins at the attached controller upkeep");

    pass_pair(&mut game);
    let token = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated { player, token } if *player == controller => Some(*token),
            _ => None,
        })
        .expect("upkeep ability creates one copied token");
    let order = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("both represented creature-lands observe the copied land entry");
    assert_eq!(order.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(order.trigger_candidates.len(), 2);
    eprintln!(
        "copied creature-land token entry trace={:?}",
        game.canonical_event_log()
    );

    assert_eq!(game.zone_of(token), Some(Zone::Battlefield));
    assert!(
        order.trigger_candidates.iter().any(|entry| {
            entry.source == token && entry.ability == "copied-creature-land-entry"
        }),
        "the copied creature-land token must enter the land-trigger ordering batch"
    );
    assert!(
        order.trigger_candidates.iter().any(|entry| {
            entry.source == creature_land && entry.ability == "copied-creature-land-entry"
        }),
        "the original creature-land must observe the same copied land-entry event"
    );
    game.submit_decision(
        controller,
        order.id,
        DecisionSelection::TriggerOrder(order.trigger_candidates.clone()),
    )
    .expect("both ordered creature-land entry triggers stack successfully");
    assert!(
        game.event_log.iter().any(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == token && *ability == "copied-creature-land-entry"
            )
        }),
        "the copied token's trigger must reach the stack without physical-definition lookup"
    );
    game.validate_invariants()
        .expect("copied creature-land token entry remains auditable");
}
