//! Red regression: a leaving controller's temporary control effect ends
//! before CR 800.4a evaluates objects controlled by that player.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, ContinuousChange, Duration, Effect, Game, GameEvent,
    ManaCost, ObjectId, PlayerId, Target, TargetRequirement, Zone,
};

const CREATURE: &str = "TST-DEPARTING-EOT-CONTROL-CREATURE";
const ACT_OF_CONTROL: &str = "TST-DEPARTING-EOT-CONTROL-SPELL";
const KILLER: &str = "TST-DEPARTING-EOT-CONTROL-KILLER";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["departing-eot-control-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn request(card: ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top(game: &mut Game) {
    for _ in 0..3 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("each player passes priority");
    }
}

#[test]
fn departing_temporary_controller_does_not_exile_opponents_creature() {
    let departing_controller = PlayerId(0);
    let creature_owner = PlayerId(1);
    let killer_controller = PlayerId(2);
    let mut game = Game::new(
        [
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                ACT_OF_CONTROL,
                CardType::Instant,
                vec![Effect::GainControlTargetUntilEndOfTurn],
            ),
            definition(
                KILLER,
                CardType::Instant,
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
    )
    .expect("fixture initializes");
    let creature = game
        .put_on_battlefield(creature_owner, CREATURE)
        .expect("opponent owns the creature");
    let act_of_control = game
        .add_card(departing_controller, ACT_OF_CONTROL, Zone::Hand)
        .expect("departing player has temporary control spell");
    let killer = game
        .add_card(killer_controller, KILLER, Zone::Hand)
        .expect("third player has lethal instant");
    game.begin_game().expect("game begins");

    game.cast_spell(
        departing_controller,
        request(act_of_control, vec![Target::Permanent(creature)]),
    )
    .expect("temporary control spell casts");
    resolve_top(&mut game);
    assert_eq!(
        game.controller_of(creature),
        Ok(departing_controller),
        "the end-of-turn control effect applied before departure"
    );

    game.pass_priority(departing_controller)
        .expect("departing controller passes");
    game.pass_priority(creature_owner)
        .expect("creature owner passes to lethal responder");
    game.cast_spell(
        killer_controller,
        request(killer, vec![Target::Player(departing_controller)]),
    )
    .expect("third player casts lethal damage");
    resolve_top(&mut game);

    eprintln!(
        "departing temporary-control red trace: zone={:?}; controller={:?}; events={:?}",
        game.zone_of(creature),
        game.controller_of(creature),
        game.canonical_event_log()
    );
    assert!(
        game.player(departing_controller)
            .expect("seat remains")
            .lost
    );
    assert_eq!(
        game.zone_of(creature),
        Some(Zone::Battlefield),
        "a departing player's temporary control effect ends before the owned creature can be exiled"
    );
    assert_eq!(game.controller_of(creature), Ok(creature_owner));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { target, from, to, .. }
            if *target == creature && *from == departing_controller && *to == creature_owner
    )));
    game.validate_invariants()
        .expect("control-effect departure leaves ordinary battlefield state");
}

#[test]
fn departing_controller_ends_source_relative_control_with_its_source() {
    let departing_controller = PlayerId(0);
    let creature_owner = PlayerId(1);
    let killer_controller = PlayerId(2);
    let mut game = Game::new(
        [
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                ACT_OF_CONTROL,
                CardType::Instant,
                vec![Effect::GainControlTargetUntilEndOfTurn],
            ),
            definition(
                KILLER,
                CardType::Instant,
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
    )
    .expect("fixture initializes");
    let control_source = game
        .put_on_battlefield(creature_owner, CREATURE)
        .expect("opponent owns the source");
    let controlled_target = game
        .put_on_battlefield(creature_owner, CREATURE)
        .expect("opponent owns the target");
    let act_of_control = game
        .add_card(departing_controller, ACT_OF_CONTROL, Zone::Hand)
        .expect("departing player has temporary control spell");
    let killer = game
        .add_card(killer_controller, KILLER, Zone::Hand)
        .expect("third player has lethal instant");
    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        control_source,
        controlled_target,
        ContinuousChange::ChangeControllerToSourceController,
        Duration::Permanent,
    )
    .expect("source-relative control effect installs");

    game.cast_spell(
        departing_controller,
        request(act_of_control, vec![Target::Permanent(control_source)]),
    )
    .expect("temporary control spell casts");
    resolve_top(&mut game);
    assert_eq!(game.controller_of(control_source), Ok(departing_controller));
    assert_eq!(
        game.controller_of(controlled_target),
        Ok(departing_controller)
    );

    game.pass_priority(departing_controller)
        .expect("departing controller passes");
    game.pass_priority(creature_owner)
        .expect("creature owner passes to lethal responder");
    game.cast_spell(
        killer_controller,
        request(killer, vec![Target::Player(departing_controller)]),
    )
    .expect("third player casts lethal damage");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(control_source), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(controlled_target), Some(Zone::Battlefield));
    assert_eq!(game.controller_of(control_source), Ok(creature_owner));
    assert_eq!(game.controller_of(controlled_target), Ok(creature_owner));
    for target in [control_source, controlled_target] {
        assert!(game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::ControllerChanged { target: changed, from, to, .. }
                if *changed == target
                    && *from == departing_controller
                    && *to == creature_owner
        )));
    }
    game.validate_invariants()
        .expect("source-relative control also reverts before departure object cleanup");
}
