//! Red regression for combat-damage prevention surviving a same-object type change.
//!
//! A resolved effect that targeted a creature continues to refer to that
//! permanent's current rules object.  It must not require the object to remain
//! a creature throughout its duration merely because creature was the target
//! restriction at resolution.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, Keyword, ManaCost, PlayerId, Step, Target,
    Zone,
};

const LAND: &str = "TST-COMBAT-PREVENTION-TYPE-CHANGE-LAND";
const ARTIFACT: &str = "TST-COMBAT-PREVENTION-TYPE-CHANGE-ARTIFACT";
const TARGET: &str = "TST-COMBAT-PREVENTION-TYPE-CHANGE-TARGET";
const PREVENTION: &str = "TST-COMBAT-PREVENTION-TYPE-CHANGE-PREVENTION";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    keywords: Vec<Keyword>,
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
        supported_rules: &["combat-prevention-type-change-probe"],
        power: creature.then_some(3),
        toughness: creature.then_some(3),
        keywords,
        effects,
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

fn add_opening_library(game: &mut Game, player: PlayerId) {
    for _ in 0..8 {
        game.add_card(player, LAND, Zone::Library)
            .expect("library fixture card exists");
    }
}

fn advance_to_attacker_declare_attackers(game: &mut Game, attacker: PlayerId) {
    while game.active_player != attacker || game.step != Step::DeclareAttackers {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public state")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .attackers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_attackers(player, &[])
                    .expect("empty attacker declaration is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .blockers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_blockers(player, &[])
                    .expect("empty blocker declaration is legal");
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority passes");
            }
        }
    }
}

fn advance_to_postcombat_main(game: &mut Game) {
    while game.step != Step::PostcombatMain {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public state")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
        }
        if game.step == Step::DeclareBlockers
            && !game
                .view_for_player(game.next_policy_player())
                .expect("combat state")
                .blockers_declared
        {
            let player = game.next_policy_player();
            game.declare_blockers(player, &[])
                .expect("empty blocker declaration is legal");
        } else {
            let player = game.priority;
            game.pass_priority(player).expect("priority passes");
        }
    }
}

#[test]
fn resolved_combat_prevention_survives_target_becoming_a_noncreature() {
    let attacker_controller = PlayerId(0);
    let caster = PlayerId(1);
    let mut game = Game::new(
        [
            definition(LAND, BTreeSet::from([CardType::Land]), vec![], vec![]),
            definition(
                ARTIFACT,
                BTreeSet::from([CardType::Artifact]),
                vec![],
                vec![],
            ),
            definition(
                TARGET,
                BTreeSet::from([CardType::Artifact, CardType::Creature]),
                vec![Keyword::Haste],
                vec![],
            ),
            definition(
                PREVENTION,
                BTreeSet::from([CardType::Instant]),
                vec![],
                vec![Effect::PreventTargetCreatureCombatDamageUntilEndOfTurn {
                    damage_target_controller_equal_to_power_if_mana_color_spent: None,
                }],
            ),
        ],
        2,
    )
    .expect("fixture builds");
    add_opening_library(&mut game, attacker_controller);
    add_opening_library(&mut game, caster);
    let copy_source = game
        .put_on_battlefield(attacker_controller, ARTIFACT)
        .expect("artifact source enters");
    let target = game
        .put_on_battlefield(attacker_controller, TARGET)
        .expect("creature target enters");
    let prevention = game
        .add_card(caster, PREVENTION, Zone::Hand)
        .expect("prevention spell enters hand");
    game.begin_game().expect("fixture begins");
    advance_to_attacker_declare_attackers(&mut game, attacker_controller);
    game.declare_attackers(attacker_controller, &[target])
        .expect("creature attacks before the prevention spell resolves");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes to responder");

    game.cast_spell(
        caster,
        CastRequest {
            card: prevention,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("combat-prevention spell casts");
    resolve_top(&mut game);
    advance_to_postcombat_main(&mut game);

    let copy_result = game.copy_permanent(target, copy_source);
    eprintln!(
        "combat prevention type-change copy result={copy_result:?}; target={:?}; events={:?}",
        game.characteristics(target),
        game.canonical_event_log()
    );
    copy_result.expect(
        "a resolved target-creature combat-prevention effect remains attached to the same live permanent after it becomes a noncreature",
    );
    assert_eq!(
        game.characteristics(target)
            .expect("copied target remains live")
            .card_types,
        BTreeSet::from([CardType::Artifact]),
        "the target now has no creature type but must retain the existing replacement record"
    );
    game.validate_invariants()
        .expect("same-object type change preserves a valid combat-prevention endpoint");
}
