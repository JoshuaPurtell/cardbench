//! Regression for a single target creature that returns from exile at end step.
//!
//! This is intentionally separate from Aura-relative linked exile: the
//! delayed return must survive an unrelated source leaving the stack and must
//! retain the target's exact exile incarnation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DelayedActionTiming, Effect, Game, GameEvent, ManaCost,
    PlayerId, Step, Target, Zone,
};

const CREATURE: &str = "TST-DELAYED-EXILE-CREATURE";
const BLINK: &str = "TST-DELAYED-EXILE";

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
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["delayed-target-creature-exile-red"],
        power: (id == CREATURE).then_some(2),
        toughness: (id == CREATURE).then_some(2),
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

fn advance_to_end_step(game: &mut Game) {
    while game.step != Step::End {
        if game.step == Step::DeclareAttackers {
            game.declare_attackers(game.active_player, &[])
                .expect("empty attacker declaration");
        }
        pass_pair(game);
    }
}

#[test]
fn targeted_creature_exile_returns_at_end_step_with_its_own_delayed_receipt() {
    let mut game = Game::new(
        vec![
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                BLINK,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::ExileTargetCreatureUntilEndStep],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let creature = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("creature enters before game");
    let blink = game
        .add_card(PlayerId(0), BLINK, Zone::Hand)
        .expect("blink enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: blink,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("blink casts");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(creature), Some(Zone::Exile));
    let exile_incarnation = game
        .object(creature)
        .expect("exiled card exists")
        .incarnation;
    let (action, group) = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::DelayedActionScheduled {
                action,
                timing: DelayedActionTiming::EndStep,
                group,
                members,
                ..
            } if members.len() == 1 && members[0].object == creature => Some((*action, *group)),
            _ => None,
        })
        .expect("target exile schedules one exact delayed return");

    advance_to_end_step(&mut game);
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert_eq!(game.controller_of(creature), Ok(PlayerId(1)));
    assert!(
        game.object(creature)
            .expect("returned creature exists")
            .incarnation
            > exile_incarnation,
        "return is a fresh battlefield object"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DelayedActionConsumed { action: actual, group: actual_group, returned }
            if *actual == action && *actual_group == group && returned == &vec![creature]
    )));
    game.validate_invariants()
        .expect("single-target delayed exile lifecycle is auditable");
}
