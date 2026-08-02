//! Green contracts for typed, battlefield-only named counters.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, CounterKind, Effect, Game, GameEvent, ManaCost, PlayerId, ReplacementEffect,
    ReplacementEffectBinding, ReplacementEventKind, RulesError, Target, Zone,
};

const ENGINE: &str = "TST-NAMED-COUNTER-ENGINE";
const DOUBLER: &str = "TST-NAMED-COUNTER-DOUBLER";
const DESTROY: &str = "TST-NAMED-COUNTER-DESTROY";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["general-named-counter-contract"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn ability(
    id: &'static str,
    targets: Vec<cardbench_magic_engine::TargetRequirement>,
    effects: Vec<Effect>,
) -> ActivatedAbility {
    ActivatedAbility {
        id,
        mana_cost: ManaCost::new(0),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets,
        effects,
    }
}

fn fixture() -> Game {
    let mut game = Game::new_with_all_bindings(
        [
            definition(ENGINE, CardType::Artifact, vec![]),
            definition(DOUBLER, CardType::Enchantment, vec![]),
            definition(
                DESTROY,
                CardType::Instant,
                vec![Effect::DestroyTargetArtifact],
            ),
        ],
        2,
        [],
        [],
        [],
        [
            ActivatedAbilityBinding {
                card_definition: ENGINE,
                ability: ability(
                    "add-charge",
                    vec![],
                    vec![Effect::AddCountersToSource {
                        counter: CounterKind::Charge,
                        amount: 2,
                    }],
                ),
            },
            ActivatedAbilityBinding {
                card_definition: ENGINE,
                ability: ability(
                    "add-quest",
                    vec![cardbench_magic_engine::TargetRequirement::Permanent],
                    vec![Effect::AddCountersToTarget {
                        counter: CounterKind::Named("quest"),
                        amount: 1,
                    }],
                ),
            },
            ActivatedAbilityBinding {
                card_definition: ENGINE,
                ability: ability(
                    "remove-three-charge",
                    vec![],
                    vec![Effect::RemoveCountersFromSource {
                        counter: CounterKind::Charge,
                        amount: 3,
                    }],
                ),
            },
        ],
    )
    .expect("counter fixture constructs");
    game.register_replacement_effect_bindings([ReplacementEffectBinding {
        source_definition: DOUBLER,
        effect: ReplacementEffect::MultiplyCounterPlacement { multiplier: 2 },
    }])
    .expect("counter quantity replacement registers before begin game");
    game
}

fn activate(
    game: &mut Game,
    source: cardbench_magic_engine::ObjectId,
    ability_id: &'static str,
    targets: Vec<Target>,
) {
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets,
        },
    )
    .expect("counter ability enters the stack");
}

fn pass_pair(game: &mut Game) -> Result<(), RulesError> {
    let first = game.priority;
    game.pass_priority(first)?;
    let second = game.priority;
    game.pass_priority(second)
}

#[test]
fn named_counter_placement_is_typed_replacement_aware_and_battlefield_only() {
    let mut game = fixture();
    let source = game
        .put_on_battlefield(PlayerId(0), ENGINE)
        .expect("source artifact starts live");
    let target = game
        .put_on_battlefield(PlayerId(0), ENGINE)
        .expect("target artifact starts live");
    let doubler = game
        .put_on_battlefield(PlayerId(0), DOUBLER)
        .expect("replacement source starts live");
    let destroy = game
        .add_card(PlayerId(0), DESTROY, Zone::Hand)
        .expect("destroy spell starts in hand");
    game.begin_game().expect("fixture game begins");
    game.clear_event_log();

    activate(&mut game, source, "add-charge", vec![]);
    pass_pair(&mut game).expect("charge ability resolves");
    activate(
        &mut game,
        source,
        "add-quest",
        vec![Target::Permanent(target)],
    );
    pass_pair(&mut game).expect("named target ability resolves");

    assert_eq!(
        game.object(source).expect("source remains live").counters,
        BTreeSet::from([(CounterKind::Charge, 4)])
            .into_iter()
            .collect(),
        "the live replacement doubles one requested pair of charge counters",
    );
    assert_eq!(
        game.object(target)
            .expect("target remains live")
            .counters
            .get(&CounterKind::Named("quest")),
        Some(&2),
        "named kinds remain declarative and use the same replacement pipeline",
    );
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::ReplacementEffectApplied {
                source: replacement_source,
                affected_player: PlayerId(0),
                event: ReplacementEventKind::CounterPlacement {
                    counter: CounterKind::Charge,
                },
                original_amount: 2,
                replacement_amount: 4,
                ..
            },
            GameEvent::CounterPlaced {
                source: counter_source,
                card,
                counter: CounterKind::Charge,
                amount: 4,
            },
        ] if *replacement_source == doubler && *counter_source == source && *card == source
    )));

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: destroy,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("destroy spell targets the named-counter permanent");
    pass_pair(&mut game).expect("destroy spell resolves");
    eprintln!(
        "named_counter_lifecycle_events={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert!(
        game.object(target)
            .expect("departed card remains an object")
            .counters
            .is_empty(),
        "counter state must clear as the permanent changes zones",
    );
    game.validate_invariants()
        .expect("named counter lifecycle preserves invariants");
}

#[test]
fn counter_removal_is_receipted_and_insufficient_removal_rolls_back_atomically() {
    let mut game = fixture();
    let source = game
        .put_on_battlefield(PlayerId(0), ENGINE)
        .expect("source artifact starts live");
    let _doubler = game
        .put_on_battlefield(PlayerId(0), DOUBLER)
        .expect("replacement source starts live");
    game.begin_game().expect("fixture game begins");
    game.clear_event_log();

    activate(&mut game, source, "add-charge", vec![]);
    pass_pair(&mut game).expect("charge ability resolves to four counters");
    activate(&mut game, source, "remove-three-charge", vec![]);
    pass_pair(&mut game).expect("sufficient removal resolves");
    assert_eq!(
        game.object(source)
            .expect("source remains live")
            .counters
            .get(&CounterKind::Charge),
        Some(&1)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterRemoved {
            source: receipt_source,
            card,
            counter: CounterKind::Charge,
            amount: 3,
        } if *receipt_source == source && *card == source
    )));

    activate(&mut game, source, "remove-three-charge", vec![]);
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let before_resolution_log = game.event_log.clone();
    let before_resolution_stack = game.stack.clone();
    let second = game.priority;
    let rejected = game.pass_priority(second);
    eprintln!(
        "named_counter_atomic_rejection result={rejected:?}; events={:?}",
        game.canonical_event_log(),
    );
    assert!(matches!(rejected, Err(RulesError::IllegalAction(_))));
    assert_eq!(
        game.object(source)
            .expect("source remains live")
            .counters
            .get(&CounterKind::Charge),
        Some(&1),
        "a failed removal cannot leak partial counter state",
    );
    assert_eq!(game.stack, before_resolution_stack);
    assert_eq!(game.event_log, before_resolution_log);
    game.validate_invariants()
        .expect("rejected removal restores a valid state-machine snapshot");
}
