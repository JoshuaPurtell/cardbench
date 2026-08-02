//! Red regression: a target-bearing triggered ability must retain the
//! affected player's damage-replacement choice at its exact damage instruction.
//!
//! The trigger resolves a prefix, a targeted damage instruction, and a suffix.
//! Competing prevention and redirection must not fall through to the legacy
//! deterministic direct-damage path merely because the source is an ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, DecisionKind, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const TRIGGER_SOURCE: &str = "TST-TRIGGERED-DAMAGE-REPLACEMENT-SOURCE";
const REDIRECTOR: &str = "TST-TRIGGERED-DAMAGE-REPLACEMENT-REDIRECTOR";
const TARGET: &str = "TST-TRIGGERED-DAMAGE-REPLACEMENT-TARGET";
const SHIELD: &str = "TST-TRIGGERED-DAMAGE-REPLACEMENT-SHIELD";
const ABILITY: &str = "attack-damage";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["triggered-damage-replacement-choice-red"],
        power: (id == TRIGGER_SOURCE || id == TARGET || id == REDIRECTOR).then_some(4),
        toughness: (id == TRIGGER_SOURCE || id == TARGET || id == REDIRECTOR).then_some(4),
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

fn advance_to_declare_attackers(game: &mut Game) {
    for player in [
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
    ] {
        game.pass_priority(player)
            .expect("advance to declare attackers");
    }
}

#[test]
#[allow(clippy::too_many_lines)] // This transcript fixes the source category and causal stack boundary together.
fn a_targeted_triggered_damage_instruction_opens_a_replacement_decision() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let trigger = TriggeredAbilityBinding {
        card_definition: TRIGGER_SOURCE,
        ability: TriggeredAbility {
            id: ABILITY,
            condition: TriggerCondition::Attacks,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Creature],
            effects: vec![
                Effect::GainLifeController { amount: 1 },
                Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Creature,
                },
                Effect::GainLifeController { amount: 2 },
            ],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(TRIGGER_SOURCE, BTreeSet::from([CardType::Creature]), vec![]),
            definition(REDIRECTOR, BTreeSet::from([CardType::Creature]), vec![]),
            definition(TARGET, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                SHIELD,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 2 }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: REDIRECTOR,
            ability: ActivatedAbility {
                id: "redirect-two",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![
                    TargetRequirement::Creature,
                    TargetRequirement::PlayerOrCreature,
                ],
                effects: vec![
                    Effect::BeginDamageRedirection { amount: 2 },
                    Effect::CompleteDamageRedirection,
                ],
            },
        }],
        [trigger],
    )
    .expect("fixture initializes");
    let source = game
        .put_on_battlefield(controller, TRIGGER_SOURCE)
        .expect("trigger source enters before game start");
    let redirector = game
        .put_on_battlefield(controller, REDIRECTOR)
        .expect("redirector enters before game start");
    let target = game
        .put_on_battlefield(controller, TARGET)
        .expect("target enters before game start");
    let shield = game
        .add_card(controller, SHIELD, Zone::Hand)
        .expect("shield starts in hand");
    game.set_entered_turn_for_setup(source, 0)
        .expect("attack trigger source predates the turn");
    game.begin_game().expect("game begins");

    game.cast_spell(
        controller,
        CastRequest {
            card: shield,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("damage shield casts");
    pass_pair(&mut game);
    game.activate_ability(
        controller,
        AbilityActivation {
            source: redirector,
            ability_id: "redirect-two",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target), Target::Player(opponent)],
        },
    )
    .expect("damage redirection ability activates");
    pass_pair(&mut game);

    advance_to_declare_attackers(&mut game);
    game.declare_attackers(controller, &[source])
        .expect("source attacks and schedules its trigger");
    let target_decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("target-bearing trigger needs its controller's target decision");
    game.submit_decision(
        controller,
        target_decision.id,
        cardbench_magic_engine::DecisionSelection::Targets(vec![Target::Permanent(target)]),
    )
    .expect("controller assigns trigger target");
    pass_pair(&mut game);

    eprintln!(
        "triggered damage replacement red trace: stack={:?}; pending={:?}; events={:?}",
        game.stack,
        game.view_for_player(controller)
            .expect("controller view")
            .pending_decision,
        game.canonical_event_log(),
    );
    let decision = game
        .view_for_player(controller)
        .expect("affected player view")
        .pending_decision
        .expect("the trigger's damage instruction must open a replacement decision");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(game.stack.len(), 1, "the resolving trigger remains live");
    game.validate_invariants()
        .expect("paused triggered replacement boundary remains valid");
}
