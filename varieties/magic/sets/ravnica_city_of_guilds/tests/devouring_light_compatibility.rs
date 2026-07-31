//! Full-fidelity contract for Devouring Light's combat-scoped exile.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Target, Zone};
use cardbench_magic_rav::card_definitions;

fn advance_to_declare_attackers(game: &mut Game) {
    game.begin_game().expect("fixture starts game");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes");
        game.pass_priority(PlayerId(1)).expect("opponent passes");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    assert_eq!(game.priority, PlayerId(0));
}

fn add_white_sources(game: &mut Game) -> [cardbench_magic_engine::ObjectId; 3] {
    [
        game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
            .expect("first Plains enters battlefield"),
        game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
            .expect("second Plains enters battlefield"),
        game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
            .expect("third Plains enters battlefield"),
    ]
}

fn activate_white_sources(game: &mut Game, sources: [cardbench_magic_engine::ObjectId; 3]) {
    for source in sources {
        game.activate_mana_ability(PlayerId(0), source, Color::White)
            .expect("Plains produces white mana");
    }
}

#[test]
fn attacking_target_is_exiled_and_event_log_is_replayable() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game initializes");
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker enters battlefield");
    let plains = add_white_sources(&mut game);
    let spell = game
        .add_card(PlayerId(0), "RAV-DEVOURING-LIGHT", Zone::Hand)
        .expect("Devouring Light enters hand");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker is established before the measured turn");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker is declared");
    activate_white_sources(&mut game, plains);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(attacker)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("combat target is legal while attacking");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("spell resolves");

    assert_eq!(game.zone_of(attacker), Some(Zone::Exile));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Exile } if *card == attacker)
    }));
    assert!(
        game.event_log
            .iter()
            .any(|event| { matches!(event, GameEvent::SpellResolved { card } if *card == spell) })
    );
    game.validate_invariants()
        .expect("exile resolution preserves engine invariants");
}

#[test]
fn noncombat_creature_is_rejected_atomically() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game initializes");
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("creature enters battlefield");
    let plains = add_white_sources(&mut game);
    let spell = game
        .add_card(PlayerId(0), "RAV-DEVOURING-LIGHT", Zone::Hand)
        .expect("Devouring Light enters hand");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[])
        .expect("empty attacker declaration opens the response window");
    activate_white_sources(&mut game, plains);
    let before = game.event_log.len();
    let error = game
        .cast_spell(
            PlayerId(0),
            CastRequest {
                card: spell,
                targets: vec![Target::Permanent(creature)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect_err("a creature outside combat is not a legal target");
    assert!(error.to_string().contains("illegal target"));
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert_eq!(game.event_log.len(), before);
    game.validate_invariants()
        .expect("rejected target leaves the game unchanged");
}
