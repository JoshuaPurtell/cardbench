//! Regression contract for expansion-neutral Aura and Equipment lifecycles.
//!
//! The original red probe established that an Equipment attach activation was
//! rejected during game construction.  These green regressions keep the
//! repaired behavior synthetic and card-name-free while covering the stack,
//! reattachment, departure, and no-cast Aura entry boundaries.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, AttachmentBinding,
    AttachmentKind, CardDefinition, CardType, CastRequest, ContinuousChange, Effect, Game,
    GameEvent, Keyword, ManaCost, ObjectId, PlayerId, RulesError, Target, TargetRequirement, Zone,
};

const EQUIPMENT: &str = "TST-GENERAL-ATTACHMENT-EQUIPMENT";
const CREATURE: &str = "TST-GENERAL-ATTACHMENT-CREATURE";
const AURA: &str = "TST-GENERAL-ATTACHMENT-AURA";
const DESTROY_CREATURE: &str = "TST-GENERAL-ATTACHMENT-DESTROY-CREATURE";
const DESTROY_ARTIFACT: &str = "TST-GENERAL-ATTACHMENT-DESTROY-ARTIFACT";

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
        supported_rules: &["general-attachment-lifecycle-contract"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn attachment_changes() -> Vec<ContinuousChange> {
    vec![ContinuousChange::ModifyPowerToughness {
        power: 1,
        toughness: 1,
    }]
}

fn fixture() -> Game {
    let mut game = Game::new_with_all_bindings(
        vec![
            definition(EQUIPMENT, CardType::Artifact, vec![]),
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                AURA,
                CardType::Enchantment,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: vec![ContinuousChange::AddKeyword(Keyword::Haste)],
                }],
            ),
            definition(
                DESTROY_CREATURE,
                CardType::Instant,
                vec![Effect::DealDamage {
                    amount: 3,
                    target: TargetRequirement::Creature,
                }],
            ),
            definition(
                DESTROY_ARTIFACT,
                CardType::Instant,
                vec![Effect::DestroyTargetArtifact],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: EQUIPMENT,
            ability: ActivatedAbility {
                id: "attach",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: true,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: attachment_changes(),
                }],
            },
        }],
    )
    .expect("typed Equipment attachment ability constructs");
    game.register_attachment_bindings([
        AttachmentBinding {
            card_definition: EQUIPMENT,
            kind: AttachmentKind::Equipment,
            target: TargetRequirement::Creature,
            changes: attachment_changes(),
            granted_activated_abilities: vec![],
        },
        AttachmentBinding {
            card_definition: AURA,
            kind: AttachmentKind::Aura,
            target: TargetRequirement::ControlledCreature,
            changes: vec![ContinuousChange::AddKeyword(Keyword::Haste)],
            granted_activated_abilities: vec![],
        },
    ])
    .expect("typed attachment bindings register before game start");
    game
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player resolves stack");
}

fn advance_to_precombat_main(game: &mut Game) {
    for _ in 0..3 {
        if game.step == cardbench_magic_engine::Step::PrecombatMain {
            return;
        }
        let first = game.priority;
        game.pass_priority(first).expect("first step pass");
        let second = game.priority;
        game.pass_priority(second).expect("second step pass");
    }
    assert_eq!(game.step, cardbench_magic_engine::Step::PrecombatMain);
}

fn attach(game: &mut Game, equipment: ObjectId, target: ObjectId) {
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: equipment,
            ability_id: "attach",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Equipment attach ability activates");
    resolve_top(game);
}

fn cast_and_resolve(game: &mut Game, spell: ObjectId, target: ObjectId) {
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("synthetic removal spell casts");
    resolve_top(game);
}

#[test]
fn equipment_attach_ability_is_a_valid_generic_activated_ability() {
    let mut game = fixture();
    let equipment = game
        .put_on_battlefield(PlayerId(0), EQUIPMENT)
        .expect("Equipment enters");
    let first = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("first creature enters");
    let second = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("second creature enters");
    game.begin_game().expect("game begins");
    game.clear_event_log();
    advance_to_precombat_main(&mut game);
    game.clear_event_log();

    attach(&mut game, equipment, first);
    assert_eq!(
        game.object(equipment)
            .expect("Equipment exists")
            .attached_to,
        Some(first)
    );
    assert_eq!(
        game.characteristics(first)
            .expect("first characteristics")
            .power,
        Some(3)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::EquipmentAttached { equipment: source, target, previous: None }
            if *source == equipment && *target == first
    )));

    attach(&mut game, equipment, second);
    assert_eq!(
        game.object(equipment)
            .expect("Equipment exists")
            .attached_to,
        Some(second)
    );
    assert_eq!(
        game.characteristics(first).expect("first resets").power,
        Some(2)
    );
    assert_eq!(
        game.characteristics(second).expect("second modified").power,
        Some(3)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::EquipmentAttached { equipment: source, target, previous: Some(old) }
            if *source == equipment && *target == second && *old == first
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { source, target, .. }
            if *source == equipment && *target == first
    )));
    game.validate_invariants()
        .expect("Equipment reattachment leaves a valid state");
}

#[test]
fn equipment_detaches_when_its_target_or_source_departs() {
    let mut target_game = fixture();
    let equipment = target_game
        .put_on_battlefield(PlayerId(0), EQUIPMENT)
        .expect("Equipment enters");
    let target = target_game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("creature enters");
    let removal = target_game
        .add_card(PlayerId(0), DESTROY_CREATURE, Zone::Hand)
        .expect("removal enters hand");
    target_game.begin_game().expect("target game begins");
    target_game.clear_event_log();
    advance_to_precombat_main(&mut target_game);
    target_game.clear_event_log();
    attach(&mut target_game, equipment, target);
    cast_and_resolve(&mut target_game, removal, target);

    assert_eq!(target_game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(target_game.zone_of(equipment), Some(Zone::Battlefield));
    assert_eq!(
        target_game
            .object(equipment)
            .expect("Equipment exists")
            .attached_to,
        None
    );
    assert!(target_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AttachmentDetached { attachment, target: detached, kind: AttachmentKind::Equipment, .. }
            if *attachment == equipment && *detached == target
    )));
    target_game
        .validate_invariants()
        .expect("target departure detaches but does not destroy Equipment");

    let mut source_game = fixture();
    let equipment = source_game
        .put_on_battlefield(PlayerId(0), EQUIPMENT)
        .expect("Equipment enters");
    let target = source_game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("creature enters");
    let removal = source_game
        .add_card(PlayerId(0), DESTROY_ARTIFACT, Zone::Hand)
        .expect("artifact removal enters hand");
    source_game.begin_game().expect("source game begins");
    source_game.clear_event_log();
    advance_to_precombat_main(&mut source_game);
    source_game.clear_event_log();
    attach(&mut source_game, equipment, target);
    cast_and_resolve(&mut source_game, removal, equipment);

    assert_eq!(source_game.zone_of(equipment), Some(Zone::Graveyard));
    assert_eq!(
        source_game
            .characteristics(target)
            .expect("target resets")
            .power,
        Some(2)
    );
    assert!(
        source_game
            .continuous_effects
            .iter()
            .all(|effect| effect.source != equipment)
    );
    source_game
        .validate_invariants()
        .expect("source departure expires linked effects");
}

#[test]
fn aura_entering_without_cast_rechecks_its_typed_controller_relative_restriction() {
    let mut game = fixture();
    let controlled = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("controlled creature enters");
    let opposing = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("opposing creature enters");
    let aura = game
        .add_card(PlayerId(0), AURA, Zone::Hand)
        .expect("Aura enters hand");
    game.begin_game().expect("game begins");
    game.clear_event_log();

    assert!(matches!(
        game.enter_attachment_without_cast(aura, opposing),
        Err(RulesError::IllegalTarget(Target::Permanent(target))) if target == opposing
    ));
    assert_eq!(game.zone_of(aura), Some(Zone::Hand));
    assert!(game.event_log.is_empty(), "illegal no-cast entry is atomic");

    game.enter_attachment_without_cast(aura, controlled)
        .expect("Aura enters attached to a legal controlled creature");
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        game.object(aura).expect("Aura exists").attached_to,
        Some(controlled)
    );
    assert!(
        game.characteristics(controlled)
            .expect("enchanted characteristics")
            .keywords
            .contains(&Keyword::Haste)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AuraAttached { aura: source, target } if *source == aura && *target == controlled
    )));
    game.validate_invariants()
        .expect("ordinary Aura invariants apply to no-cast entry");
}
