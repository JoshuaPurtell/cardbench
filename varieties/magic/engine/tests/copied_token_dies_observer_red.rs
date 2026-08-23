//! Red regression: copied tokens observe another creature dying.
//!
//! The physical and token copy share one definition-bound `AnotherCreatureDies`
//! trigger, so a later unrelated creature death must create one APNAP ordering
//! batch containing both sources.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, CastRequest, DecisionKind,
    DecisionSelection, Effect, Game, GameEvent, ManaCost, PlayerId, Target, TargetRequirement,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const OBSERVER: &str = "TST-COPIED-TOKEN-DIES-OBSERVER";
const VICTIM: &str = "TST-COPIED-TOKEN-DIES-VICTIM";
const COPY_AURA: &str = "TST-COPIED-TOKEN-DIES-AURA";
const KILL: &str = "TST-COPIED-TOKEN-DIES-KILL";

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
        supported_rules: &["copied-token-dies-observer-red"],
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
#[allow(clippy::too_many_lines)] // The copy/dies/ordered-trigger transcript is the regression contract.
fn copied_token_observes_another_creature_dying() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(OBSERVER, BTreeSet::from([CardType::Creature]), vec![]),
            definition(VICTIM, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                COPY_AURA,
                BTreeSet::from([CardType::Enchantment]),
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: vec![],
                }],
            ),
            definition(
                KILL,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DestroyTargetNonblackCreature],
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
                card_definition: OBSERVER,
                ability: TriggeredAbility {
                    id: "another-creature-dies",
                    condition: TriggerCondition::AnotherCreatureDies,
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
    let observer = game
        .put_on_battlefield(controller, OBSERVER)
        .expect("observer setup");
    let victim = game
        .put_on_battlefield(opponent, VICTIM)
        .expect("victim setup");
    let aura = game
        .add_card(controller, COPY_AURA, Zone::Hand)
        .expect("Aura setup");
    game.enter_attachment_without_cast(aura, observer)
        .expect("pregame Aura setup attaches");
    let kill = game
        .add_card(controller, KILL, Zone::Hand)
        .expect("kill spell setup");
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
        .expect("upkeep ability creates one copied observer token");

    let death_start = game.event_log.len();
    game.cast_spell(
        controller,
        CastRequest {
            card: kill,
            targets: vec![Target::Permanent(victim)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("kill spell casts");
    pass_pair(&mut game);
    eprintln!(
        "copied token dies-observer trace={:?}",
        &game.canonical_event_log()[death_start..]
    );

    assert_eq!(game.zone_of(victim), Some(Zone::Graveyard));
    let order = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("the original and copied observer require trigger ordering");
    assert_eq!(order.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(order.trigger_candidates.len(), 2);
    assert!(
        order
            .trigger_candidates
            .iter()
            .any(|entry| entry.source == observer && entry.ability == "another-creature-dies")
    );
    assert!(
        order
            .trigger_candidates
            .iter()
            .any(|entry| entry.source == token && entry.ability == "another-creature-dies")
    );

    game.submit_decision(
        controller,
        order.id,
        DecisionSelection::TriggerOrder(order.trigger_candidates.clone()),
    )
    .expect("both observer triggers stack successfully");
    pass_pair(&mut game);
    pass_pair(&mut game);
    assert_eq!(
        game.player(controller)
            .expect("controller remains live")
            .life,
        22,
        "the original and copied token each resolve their dies observer"
    );
    game.validate_invariants()
        .expect("copied-token dies observer lifecycle is auditable");
}
