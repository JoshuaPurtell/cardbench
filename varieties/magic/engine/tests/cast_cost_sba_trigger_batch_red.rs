//! Red regression: a trigger from a cast's sacrifice cost and one created by
//! the immediate post-cost SBA must be placed as one simultaneous batch.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AdditionalSpellCost, AdditionalSpellCostBinding, CardDefinition, CardType, CastRequest,
    ContinuousChange, DecisionKind, Effect, Game, GameEvent, Keyword, ManaCost, PlayerId,
    StaticContinuousEffectBinding, Target, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const ANTHEM: &str = "TST-CAST-COST-SBA-ANTHEM";
const FRAGILE: &str = "TST-CAST-COST-SBA-FRAGILE";
const SPELL: &str = "TST-CAST-COST-SBA-SPELL";

fn creature(
    id: &'static str,
    power: i16,
    toughness: i16,
    keywords: Vec<Keyword>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["cast-cost-sba-trigger-batch-red"],
        power: Some(power),
        toughness: Some(toughness),
        keywords,
        effects: vec![],
    }
}

fn spell() -> CardDefinition {
    CardDefinition {
        id: SPELL,
        name: SPELL,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["cast-cost-sba-trigger-batch-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::GainLifeController { amount: 1 }],
    }
}

fn dies_trigger(id: &'static str) -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: id,
        ability: TriggeredAbility {
            id: if id == ANTHEM {
                "anthem-dies"
            } else {
                "fragile-dies"
            },
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 1 }],
        },
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second pass resolves the stack");
}

#[test]
fn cast_cost_and_resulting_sba_dies_triggers_share_one_apnap_batch() {
    let mut game = Game::new_with_all_bindings_triggers_and_static_continuous_effects(
        [
            creature(ANTHEM, 1, 1, vec![]),
            creature(FRAGILE, 0, 0, vec![Keyword::Flash]),
            spell(),
        ],
        2,
        [],
        [],
        [AdditionalSpellCostBinding {
            card_definition: SPELL,
            cost: AdditionalSpellCost::SacrificeControlledCreature,
        }],
        [],
        [dies_trigger(ANTHEM), dies_trigger(FRAGILE)],
        [StaticContinuousEffectBinding {
            card_definition: ANTHEM,
            change: ContinuousChange::OtherControlledCreaturesModifyPowerToughness {
                power: 0,
                toughness: 1,
            },
        }],
    )
    .expect("fixture constructs");
    let anthem = game
        .put_on_battlefield(PlayerId(0), ANTHEM)
        .expect("anthem enters before the game starts");
    let fragile = game
        .add_card(PlayerId(0), FRAGILE, Zone::Hand)
        .expect("fragile flash creature begins in hand");
    let spell = game
        .add_card(PlayerId(0), SPELL, Zone::Hand)
        .expect("sacrifice spell begins in hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: fragile,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("flash creature casts during upkeep");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(fragile), Some(Zone::Battlefield));
    assert_eq!(
        game.characteristics(fragile)
            .expect("fragile lives")
            .toughness,
        Some(1)
    );
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::SacrificePermanent(anthem)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("sacrifice-cost spell casts");

    let view = game.view_for_player(PlayerId(0)).expect("controller view");
    eprintln!(
        "cast-cost SBA trigger batch trace: pending={:?}; stack={:?}; events={:?}",
        view.pending_decision,
        game.stack,
        game.canonical_event_log(),
    );
    let decision = view
        .pending_decision
        .expect("both same-controller dies triggers require one ordering decision");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(decision.trigger_candidates.len(), 2);
    assert!(
        decision
            .trigger_candidates
            .iter()
            .any(|entry| entry.source == anthem)
    );
    assert!(
        decision
            .trigger_candidates
            .iter()
            .any(|entry| entry.source == fragile)
    );
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::TriggeredAbilityStacked { .. })),
        "trigger placement waits for the shared ordering decision; events={:?}",
        game.canonical_event_log(),
    );
    game.validate_invariants()
        .expect("simultaneous cost and SBA trigger batch remains auditable");
}
