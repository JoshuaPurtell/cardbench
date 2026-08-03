//! Red regression: copied tokens retain their controller-end-step triggers.
//!
//! A token that copies a permanent inherits the copied card's triggered
//! abilities.  The end-step dispatcher must therefore look through its copied
//! layer-one definition, just as the upkeep dispatcher already does.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, DecisionKind, DecisionSelection,
    Effect, Game, GameEvent, ManaCost, PlayerId, Step, TargetRequirement, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-COPIED-TOKEN-END-STEP-CREATURE";
const COPY_AURA: &str = "TST-COPIED-TOKEN-END-STEP-AURA";

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
        supported_rules: &["copied-token-end-step-trigger-red"],
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
#[allow(clippy::too_many_lines)] // The copy + ordered end-step transcript is the regression contract.
fn copied_token_stacks_its_controller_end_step_trigger() {
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
                card_definition: CREATURE,
                ability: TriggeredAbility {
                    id: "copied-creature-controller-end-step",
                    condition: TriggerCondition::BeginningOfControllerEndStep,
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
    let creature = game
        .put_on_battlefield(controller, CREATURE)
        .expect("creature setup");
    let aura = game
        .add_card(controller, COPY_AURA, Zone::Hand)
        .expect("Aura setup");
    game.enter_attachment_without_cast(aura, creature)
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
    assert_eq!(game.zone_of(token), Some(Zone::Battlefield));

    let end_start = game.event_log.len();
    advance_to_end_step(&mut game);
    eprintln!(
        "copied token end-step dispatch trace={:?}",
        &game.canonical_event_log()[end_start..]
    );
    let order = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("the original and copied creature require controller trigger ordering");
    assert_eq!(order.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(order.trigger_candidates.len(), 2);
    assert!(order.trigger_candidates.iter().any(|entry| {
        entry.source == creature && entry.ability == "copied-creature-controller-end-step"
    }));
    assert!(order.trigger_candidates.iter().any(|entry| {
        entry.source == token && entry.ability == "copied-creature-controller-end-step"
    }));

    game.submit_decision(
        controller,
        order.id,
        DecisionSelection::TriggerOrder(order.trigger_candidates.clone()),
    )
    .expect("copied token trigger orders and stacks successfully");
    pass_pair(&mut game);
    pass_pair(&mut game);
    assert_eq!(
        game.player(controller).expect("controller remains live").life,
        22,
        "both original and copied creature end-step triggers resolve"
    );
    game.validate_invariants()
        .expect("copied token end-step trigger lifecycle is auditable");
}
