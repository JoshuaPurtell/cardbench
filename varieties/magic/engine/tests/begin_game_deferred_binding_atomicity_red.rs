//! Red regression: a rejected `begin_game` must not consume the setup boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Effect, Game, ManaCost, PlayerId,
    RulesError, TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

const AURA: &str = "TST-DEFERRED-START-AURA";

fn aura_definition() -> CardDefinition {
    CardDefinition {
        id: AURA,
        name: AURA,
        set_code: "TST",
        mana_cost: ManaCost::new(1),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &["test-only-aura"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![],
        }],
    }
}

fn attached_creature_end_step_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: AURA,
        ability: TriggeredAbility {
            id: "attached-creature-end-step",
            condition: TriggerCondition::BeginningOfAttachedCreaturesControllerEndStep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::SacrificeAttachedCreatureUnlessItAttackedThisTurn],
        },
    }
}

fn aura_binding() -> AttachmentBinding {
    AttachmentBinding {
        card_definition: AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }
}

#[test]
fn rejected_start_preserves_setup_for_deferred_attachment_binding_repair() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        [aura_definition()],
        2,
        [],
        [],
        [],
        [],
        [attached_creature_end_step_trigger()],
    )
    .expect("the cross-binding requirement is deliberately deferred until game start");
    let before_events = game.canonical_event_log();

    let result = game.begin_game();
    eprintln!(
        "rejected-start trace: result={result:?}; turn={}; step={:?}; active={:?}; priority={:?}; events={:?}",
        game.turn,
        game.step,
        game.active_player,
        game.priority,
        game.canonical_event_log(),
    );
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "attachment-relative end-step trigger lacks an Aura binding"
        ))
    ));
    assert_eq!(
        game.canonical_event_log(),
        before_events,
        "a rejected start must not append a partial turn receipt",
    );

    game.register_attachment_bindings([aura_binding()])
        .expect("a rejected start must leave the setup boundary available for repair");
    game.begin_game()
        .expect("the repaired configuration starts from a clean initial boundary");
    assert_eq!(game.active_player, PlayerId(0));
    game.validate_invariants()
        .expect("the retried start leaves a valid live state machine");
}
