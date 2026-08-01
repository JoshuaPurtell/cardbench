//! Red regression: protection from a color must make a colored spell target illegal.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, Effect, Game, Keyword, ManaCost,
    PlayerId, RulesError, Step, Target, Zone,
};

const PROTECTED: &str = "PROTECTION-TARGET";
const RED_BOLT: &str = "PROTECTION-RED-BOLT";
const RED_SWEEP: &str = "PROTECTION-RED-SWEEP";
const RED_ATTACKER: &str = "PROTECTION-RED-ATTACKER";

fn pass_to_declare_attackers(game: &mut Game) {
    for _ in 0..4 {
        game.pass_priority(game.priority)
            .expect("priority pass reaches attacker declaration");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: PROTECTED,
            name: PROTECTED,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["protection-from-red"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![Keyword::Protection(Color::Red)],
            effects: vec![],
        },
        CardDefinition {
            id: RED_BOLT,
            name: RED_BOLT,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["protection-targeting-probe"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 2,
                target: cardbench_magic_engine::TargetRequirement::Creature,
            }],
        },
        CardDefinition {
            id: RED_SWEEP,
            name: RED_SWEEP,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["protection-damage-prevention-probe"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 1 }],
        },
        CardDefinition {
            id: RED_ATTACKER,
            name: RED_ATTACKER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["protection-combat-probe"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![Keyword::Haste],
            effects: vec![],
        },
    ]
}

#[test]
fn protection_from_red_prevents_non_targeted_red_damage() {
    let caster = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("fixture game initializes");
    let protected = game
        .put_on_battlefield(defender, PROTECTED)
        .expect("protected creature enters");
    let sweep = game
        .add_card(caster, RED_SWEEP, Zone::Hand)
        .expect("red sweep enters hand");

    game.cast_spell(
        caster,
        CastRequest {
            card: sweep,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the non-targeted red sweep is cast");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(defender)
        .expect("defender passes and resolves");

    assert_eq!(game.object(protected).expect("protected object").damage, 0);
    assert_eq!(game.player(defender).expect("defender state").life, 19);
    let events = game.canonical_event_log();
    assert!(
        events.iter().any(|event| event.contains(&format!(
            "DamagePrevented {{ source: {sweep:?}, target: Permanent({protected:?}), amount: 1 }}"
        ))),
        "protected creature damage must be prevented; events={events:?}"
    );
    game.validate_invariants()
        .expect("protection damage replacement leaves a valid state");
}

#[test]
fn protection_from_red_rejects_red_combat_block() {
    let attacker_controller = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("fixture game initializes");
    let attacker = game
        .put_on_battlefield(attacker_controller, RED_ATTACKER)
        .expect("red attacker enters");
    let blocker = game
        .put_on_battlefield(defender, PROTECTED)
        .expect("protected blocker enters");
    game.turn = 2;
    pass_to_declare_attackers(&mut game);
    game.declare_attackers(attacker_controller, &[attacker])
        .expect("red attacker is legal");
    game.pass_priority(attacker_controller)
        .expect("attacker passes after declaration");
    game.pass_priority(defender)
        .expect("defender receives blocker priority");
    let events_before = game.canonical_event_log().clone();

    let result = game.declare_blockers(defender, &[CombatBlock { attacker, blocker }]);
    assert!(
        matches!(result, Err(RulesError::IllegalAction("illegal blocker"))),
        "protection from red must reject the red attacker/blocker assignment; result={result:?}"
    );
    assert_eq!(
        game.canonical_event_log(),
        events_before,
        "an illegal protection block must not emit BlockersDeclared"
    );
    game.validate_invariants()
        .expect("failed protection block leaves a valid combat state");
}

#[test]
fn protection_from_red_rejects_red_spell_target_before_cast() {
    let caster = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("fixture game initializes");
    let protected = game
        .put_on_battlefield(caster, PROTECTED)
        .expect("protected creature enters");
    let bolt = game
        .add_card(caster, RED_BOLT, Zone::Hand)
        .expect("red spell enters hand");

    let result = game.cast_spell(
        caster,
        CastRequest {
            card: bolt,
            targets: vec![Target::Permanent(protected)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    eprintln!(
        "protection target red result: {result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        result.is_err(),
        "a red spell must not be cast targeting a permanent with protection from red"
    );
    assert!(
        game.stack.is_empty(),
        "illegal target must not reach the stack"
    );
    assert!(
        game.canonical_event_log().is_empty(),
        "illegal target rejection must not emit gameplay receipts"
    );
    game.validate_invariants()
        .expect("the rejected target leaves a valid fixture state");
}
