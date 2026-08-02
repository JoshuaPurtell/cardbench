//! Regression for the optional branch of a spell-copy target choice.
//!
//! “May choose new targets” is not a requirement to retarget. The controller
//! must be able to retain every original target after seeing the public,
//! no-priority target-choice boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, PlayerId, Target, TargetRequirement, Zone,
};

const PING: &str = "TST-MAY-COPY-PING";
const COPY: &str = "TST-MAY-COPY";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["spell-copy-may-decline-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn request(card: cardbench_magic_engine::ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

#[test]
fn spell_copy_may_decline_retargting_and_retains_original_targets() {
    let mut game = Game::new(
        vec![
            definition(
                PING,
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Player,
                }],
            ),
            definition(
                COPY,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: true,
                }],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let ping = game
        .add_card(PlayerId(0), PING, Zone::Hand)
        .expect("ping enters hand");
    let copy = game
        .add_card(PlayerId(0), COPY, Zone::Hand)
        .expect("copy enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        request(ping, vec![Target::Player(PlayerId(1))]),
    )
    .expect("ping casts");
    game.cast_spell(PlayerId(0), request(copy, vec![Target::Spell(ping)]))
        .expect("copy effect casts");
    resolve_top(&mut game);

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("optional target choice opens");
    assert_eq!(decision.kind, DecisionKind::SpellCopyTargets);
    assert_eq!(decision.min_selections, 0, "retargeting is optional");
    assert_eq!(decision.max_selections, 1);
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Targets(vec![]),
    )
    .expect("controller may decline to retarget the copied spell");

    assert_eq!(game.zone_of(copy), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopied {
            original,
            retargeted: false,
            ..
        } if *original == ping
    )));
    resolve_top(&mut game);
    resolve_top(&mut game);
    assert_eq!(game.players[1].life, 16, "copy retained the original target");
    game.validate_invariants()
        .expect("declined retarget remains auditable");
}
