//! Red regression for a target creature's all-combat-damage prevention effect.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost,
    ManaPaymentSelection, PlayerId, Step, Target, Zone,
};

const LAND: &str = "TST-COMBAT-PREVENTION-LAND";
const RED_LAND: &str = "TST-COMBAT-PREVENTION-RED-LAND";
const ATTACKER: &str = "TST-COMBAT-PREVENTION-ATTACKER";
const SHIELD: &str = "TST-COMBAT-PREVENTION-SHIELD";

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: LAND,
            name: LAND,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::White]),
            card_types: BTreeSet::from([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["opening-library-fixture"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: ATTACKER,
            name: ATTACKER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["combat-fixture"],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: RED_LAND,
            name: RED_LAND,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Red]),
            card_types: BTreeSet::from([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["mana-fixture"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: SHIELD,
            name: SHIELD,
            set_code: "TST",
            mana_cost: ManaCost::with_colors(2, [Color::White]),
            colors: BTreeSet::from([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["conditional-combat-damage-prevention"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::PreventTargetCreatureCombatDamageUntilEndOfTurn {
                damage_target_controller_equal_to_power_if_mana_color_spent: Some(Color::Red),
            }],
        },
    ]
}

fn add_opening_library(game: &mut Game, player: PlayerId) {
    for _ in 0..8 {
        game.add_card(player, LAND, Zone::Library)
            .expect("library fixture card exists");
    }
}

fn advance_to_opponent_declare_attackers(game: &mut Game) {
    while game.active_player != PlayerId(1) || game.step != Step::DeclareAttackers {
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

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
#[allow(clippy::too_many_lines)] // The complete combat window protects the state-machine boundary.
fn conditional_target_combat_prevention_uses_the_explicit_mana_receipt() {
    let caster = PlayerId(0);
    let attacker_controller = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("synthetic fixture builds");
    add_opening_library(&mut game, caster);
    add_opening_library(&mut game, attacker_controller);
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker setup");
    let shield = game
        .add_card(caster, SHIELD, Zone::Hand)
        .expect("shield setup");
    let white_land = game
        .put_on_battlefield(caster, LAND)
        .expect("white mana source setup");
    let first_red_land = game
        .put_on_battlefield(caster, RED_LAND)
        .expect("first red mana source setup");
    let second_red_land = game
        .put_on_battlefield(caster, RED_LAND)
        .expect("second red mana source setup");
    game.begin_game().expect("fixture begins");
    advance_to_opponent_declare_attackers(&mut game);

    game.declare_attackers(attacker_controller, &[attacker])
        .expect("attacker declares");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes to responder");
    for (land, color) in [
        (white_land, Color::White),
        (first_red_land, Color::Red),
        (second_red_land, Color::Red),
    ] {
        game.activate_mana_ability(caster, land, color)
            .expect("mana source produces the selected spell payment");
    }
    game.cast_spell_with_mana_spend(
        caster,
        CastRequest {
            card: shield,
            targets: vec![Target::Permanent(attacker)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![Color::Red, Color::Red],
            hybrid: vec![],
        },
    )
    .expect("conditional combat-prevention spell casts");
    pass_pair(&mut game);

    assert_eq!(
        game.player(attacker_controller)
            .expect("attacker controller exists")
            .life,
        17,
        "red spent in the cast receipt deals the target's current power to its controller"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CombatDamagePreventionCreated {
            source,
            creature,
            expires_turn,
        } if *source == shield && *creature == attacker && *expires_turn == game.turn
    )));

    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(caster, &[])
        .expect("no blockers declared");
    pass_pair(&mut game);

    assert_eq!(
        game.player(caster).expect("caster exists").life,
        20,
        "the target creature's combat damage is fully prevented"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CombatDamagePrevented {
            source,
            prevented_by,
            target: Target::Player(player),
            amount: 3,
        } if *source == attacker && *prevented_by == shield && *player == caster
    )));
    eprintln!(
        "conditional combat-prevention trace={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("conditional combat prevention preserves invariant state");
}
