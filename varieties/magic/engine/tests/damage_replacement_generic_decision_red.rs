//! Red regression for migrating prospective damage replacement into the
//! generic affected-player replacement decision boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, DamageReplacementChoice, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, PlayerId, PolicyAction, ReplacementChoice, Target, TargetRequirement,
    Zone,
};

const REDIRECTOR: &str = "TST-GENERIC-DAMAGE-REDIRECTOR";
const TARGET: &str = "TST-GENERIC-DAMAGE-TARGET";
const SHIELD: &str = "TST-GENERIC-DAMAGE-SHIELD";
const BOLT: &str = "TST-GENERIC-DAMAGE-BOLT";

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
        supported_rules: &["generic-damage-replacement-decision-red"],
        power: (id == TARGET || id == REDIRECTOR).then_some(4),
        toughness: (id == TARGET || id == REDIRECTOR).then_some(4),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

#[test]
#[allow(clippy::too_many_lines)] // The fixture and its public replacement trace are intentionally co-located.
fn damage_prevention_and_redirection_use_the_generic_replacement_decision() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        vec![
            definition(REDIRECTOR, BTreeSet::from([CardType::Creature]), vec![]),
            definition(TARGET, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                SHIELD,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 2 }],
            ),
            definition(
                BOLT,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Creature,
                }],
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
    )
    .expect("fixture initializes");
    let redirector = game
        .put_on_battlefield(caster, REDIRECTOR)
        .expect("redirector enters");
    let target = game
        .put_on_battlefield(caster, TARGET)
        .expect("target enters");

    let shield = game.add_card(caster, SHIELD, Zone::Hand).expect("shield");
    game.cast_spell(
        caster,
        CastRequest {
            card: shield,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("shield casts");
    pass_pair(&mut game);
    game.activate_ability(
        caster,
        AbilityActivation {
            source: redirector,
            ability_id: "redirect-two",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target), Target::Player(opponent)],
        },
    )
    .expect("redirection activates");
    pass_pair(&mut game);

    let bolt = game.add_card(caster, BOLT, Zone::Hand).expect("bolt");
    game.cast_spell(
        caster,
        CastRequest {
            card: bolt,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("bolt casts");
    pass_pair(&mut game);

    let decision = game
        .view_for_player(caster)
        .expect("affected player view")
        .pending_decision
        .expect("damage replacements use the generic decision surface");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(decision.replacement_candidates.len(), 2);
    let redirect = decision
        .replacement_candidates
        .iter()
        .copied()
        .find(|choice| {
            matches!(
                choice,
                ReplacementChoice::Damage(DamageReplacementChoice::Redirect { .. })
            )
        })
        .expect("redirection is a generic replacement option");

    game.submit_policy_move(
        caster,
        "generic-damage-replacement-decision-red",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Replacements(vec![redirect]),
        },
    )
    .expect("affected player selects the redirection through the generic action");

    eprintln!(
        "generic damage replacement trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.player(opponent).expect("opponent").life, 18);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionOpened { decision: opened, kind: DecisionKind::Replacement, .. }
            if *opened == decision.id
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionCompleted { decision: completed, kind: DecisionKind::Replacement, .. }
            if *completed == decision.id
    )));
    game.validate_invariants()
        .expect("generic damage replacement decision is invariant-safe");
}
