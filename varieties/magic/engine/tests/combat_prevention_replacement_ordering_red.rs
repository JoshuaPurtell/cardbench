//! Red regression: an affected player must order concurrent combat prevention
//! and source-bound combat-damage replacement effects before either commits.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DamageReplacementEffect, DamageReplacementEffectBinding,
    DecisionKind, Effect, Game, ManaCost, PlayerId, Step, Target, Zone,
};

const REPLACER: &str = "COMBAT-PREVENTION-ORDERING-REPLACER";
const PREVENTION: &str = "COMBAT-PREVENTION-ORDERING-PREVENTION";
const LIBRARY_CARD: &str = "COMBAT-PREVENTION-ORDERING-LIBRARY";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["combat-prevention-replacement-ordering-probe"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

fn pass_to(game: &mut Game, step: Step) {
    while game.step != step {
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("priority advances the deterministic combat fixture");
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
#[allow(clippy::too_many_lines)] // The complete stack and combat transcript is the regression.
fn affected_player_orders_combat_prevention_and_source_replacement() {
    let attacker_controller = PlayerId(0);
    let affected_player = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                REPLACER,
                BTreeSet::from([CardType::Creature]),
                Some(4),
                Some(4),
                vec![],
            ),
            definition(
                PREVENTION,
                BTreeSet::from([CardType::Instant]),
                None,
                None,
                vec![Effect::PreventTargetCreatureCombatDamageUntilEndOfTurn {
                    damage_target_controller_equal_to_power_if_mana_color_spent: None,
                }],
            ),
            definition(
                LIBRARY_CARD,
                BTreeSet::from([CardType::Artifact]),
                None,
                None,
                vec![],
            ),
        ],
        2,
    )
    .expect("two-player fixture initializes");
    game.register_damage_replacement_effect_bindings([DamageReplacementEffectBinding {
        source_definition: REPLACER,
        effect: DamageReplacementEffect::ReplaceCombatDamageToPlayerWithMillAndCounters,
    }])
    .expect("source combat replacement binds before game start");
    let attacker = game
        .put_on_battlefield(attacker_controller, REPLACER)
        .expect("attacker begins on the battlefield");
    let prevention = game
        .add_card(affected_player, PREVENTION, Zone::Hand)
        .expect("defender has the prevention spell");
    for _ in 0..4 {
        game.add_card(affected_player, LIBRARY_CARD, Zone::Library)
            .expect("affected player has a four-card library");
    }
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker predates this turn");
    game.begin_game().expect("fixture starts");
    pass_to(&mut game, Step::DeclareAttackers);
    game.declare_attackers(attacker_controller, &[attacker])
        .expect("attacker attacks");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes to defender");
    game.cast_spell(
        affected_player,
        CastRequest {
            card: prevention,
            targets: vec![Target::Permanent(attacker)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("defender creates a combat prevention replacement");
    pass_pair(&mut game);
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(affected_player, &[])
        .expect("defender declares no blockers");

    // The two priority passes start ordinary combat damage. Both the
    // prevention and the source's mill/counter replacement apply to the same
    // prospective packet, so the affected player must choose an order before
    // the engine mills cards or records combat prevention.
    pass_pair(&mut game);

    let decision = game
        .view_for_player(affected_player)
        .expect("affected player has a view")
        .pending_decision
        .expect("affected player must order combat prevention and replacement");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(game.next_policy_player(), affected_player);
    assert_eq!(
        game.player(affected_player)
            .expect("affected player remains live")
            .library
            .len(),
        4,
        "no replacement may commit before the affected player chooses",
    );
}
