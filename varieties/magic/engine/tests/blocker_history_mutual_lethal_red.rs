//! Red regression for combat block provenance after mutual lethal damage.
//!
//! A legal attacker/blocker pair remains historical combat provenance even
//! after both creatures die in the same combat-damage state-based-action
//! batch.  Advancing that legal combat must not roll back the final priority
//! pass because the objects now have new zone incarnations.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, Game, GameEvent, ManaCost, PlayerId,
    PolicyAction, Step, Zone,
};

const POLICY_ID: &str = "engine.block-history-mutual-lethal.v1";
const LAND: &str = "BLOCK-HISTORY-LAND";
const ATTACKER: &str = "BLOCK-HISTORY-ATTACKER";
const BLOCKER: &str = "BLOCK-HISTORY-BLOCKER";

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: LAND,
            name: "Block History Land",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Green]),
            card_types: BTreeSet::from([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["basic-mana"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: ATTACKER,
            name: "Block History Attacker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics", "combat"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: BLOCKER,
            name: "Block History Blocker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics", "combat"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn add_library(game: &mut Game, player: PlayerId) {
    for _ in 0..8 {
        game.add_card(player, LAND, Zone::Library)
            .expect("fixture land enters library");
    }
}

fn advance_to_declare_attackers(game: &mut Game) {
    while game.turn != 3 || game.step != Step::DeclareAttackers {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw resolves");
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view")
                    .attackers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_attackers(player, &[])
                    .expect("empty attack declaration advances combat");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view")
                    .blockers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_blockers(player, &[])
                    .expect("empty block declaration advances combat");
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority pass advances step");
            }
        }
    }
}

fn submit(game: &mut Game, player: PlayerId, action: PolicyAction) -> Result<(), String> {
    game.submit_policy_move(player, POLICY_ID, action)
        .map_err(|error| format!("{error}"))
}

#[test]
fn mutual_lethal_combat_preserves_declared_block_history_through_sbas() {
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    add_library(&mut game, PlayerId(0));
    add_library(&mut game, PlayerId(1));
    let attacker = game
        .put_on_battlefield(PlayerId(0), ATTACKER)
        .expect("attacker enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), BLOCKER)
        .expect("blocker enters");
    game.begin_game().expect("game begins");
    advance_to_declare_attackers(&mut game);
    game.clear_event_log();

    submit(
        &mut game,
        PlayerId(0),
        PolicyAction::DeclareAttackers {
            attackers: vec![attacker],
        },
    )
    .expect("attacker declaration is accepted");
    for player in [PlayerId(0), PlayerId(1)] {
        submit(&mut game, player, PolicyAction::PassPriority)
            .expect("priority reaches blocker declaration");
    }
    assert_eq!(game.step, Step::DeclareBlockers);
    submit(
        &mut game,
        PlayerId(1),
        PolicyAction::DeclareBlockers {
            assignments: vec![CombatBlock { attacker, blocker }],
        },
    )
    .expect("blocker declaration is accepted");

    submit(&mut game, PlayerId(0), PolicyAction::PassPriority)
        .expect("first post-block priority pass is accepted");
    let result = submit(&mut game, PlayerId(1), PolicyAction::PassPriority);

    assert_eq!(
        result,
        Ok(()),
        "mutual lethal combat must reach its state-based-action fixed point without rolling back the final pass; events={:#?}",
        game.event_log
    );
    assert_eq!(game.zone_of(attacker), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(blocker), Some(Zone::Graveyard));
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::StateBasedAction { card, reason: "creature has lethal damage" }
                if *card == attacker
        )),
        "attacker lethal-SBA receipt must survive the combat transition"
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::StateBasedAction { card, reason: "creature has lethal damage" }
                if *card == blocker
        )),
        "blocker lethal-SBA receipt must survive the combat transition"
    );
    game.validate_invariants()
        .expect("historical block provenance remains valid after both combatants leave");
}
