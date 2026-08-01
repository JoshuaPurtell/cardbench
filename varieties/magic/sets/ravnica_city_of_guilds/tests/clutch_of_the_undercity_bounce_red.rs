//! Red regression for Clutch of the Undercity's omitted front face.

use cardbench_magic_engine::{
    CastRequest, Color, Effect, Game, GameEvent, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::card_definitions;

fn game() -> Game {
    Game::new(card_definitions(), 2).expect("RAV game builds")
}

fn add_payment(game: &mut Game) {
    game.grant_mana(PlayerId(0), Color::Blue, 2)
        .expect("blue mana");
    game.grant_mana(PlayerId(0), Color::Black, 1)
        .expect("black mana");
}

#[test]
fn clutch_bounces_a_permanent_then_uses_its_last_battlefield_controller_for_life_loss() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CLUTCH-OF-THE-UNDERCITY")
        .expect("Clutch definition exists");
    assert_eq!(
        definition.effects,
        vec![Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 3 }],
        "Clutch must expose its typed targeted permanent-bounce front face"
    );

    let mut legal = game();
    legal.step = Step::PrecombatMain;
    let clutch = legal
        .add_card(PlayerId(0), "RAV-CLUTCH-OF-THE-UNDERCITY", Zone::Hand)
        .expect("Clutch setup");
    let target = legal
        .put_on_battlefield(PlayerId(1), "RAV-DIMIR-SIGNET")
        .expect("artifact permanent target");
    add_payment(&mut legal);
    legal
        .cast_spell(
            PlayerId(0),
            CastRequest {
                card: clutch,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect("Clutch accepts any permanent");
    legal.pass_priority(PlayerId(0)).expect("caster passes");
    legal.pass_priority(PlayerId(1)).expect("opponent passes");
    println!("Clutch bounce trace: {:?}", legal.event_log);
    assert_eq!(legal.zone_of(target), Some(Zone::Hand));
    assert_eq!(legal.players[1].life, 17);
    assert!(legal.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeLost { source, player, amount }
            if *source == clutch && *player == PlayerId(1) && *amount == 3
    )));
    legal
        .validate_invariants()
        .expect("Clutch bounce preserves invariants");

    let mut illegal = game();
    illegal.step = Step::PrecombatMain;
    let clutch = illegal
        .add_card(PlayerId(0), "RAV-CLUTCH-OF-THE-UNDERCITY", Zone::Hand)
        .expect("Clutch setup");
    add_payment(&mut illegal);
    let before = illegal.event_log.clone();
    assert!(
        illegal
            .cast_spell(
                PlayerId(0),
                CastRequest {
                    card: clutch,
                    targets: vec![Target::Player(PlayerId(1))],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
            )
            .is_err(),
        "Clutch cannot target a player"
    );
    assert_eq!(illegal.zone_of(clutch), Some(Zone::Hand));
    assert_eq!(illegal.players[1].life, 20);
    assert_eq!(illegal.event_log, before);
    illegal
        .validate_invariants()
        .expect("rejected Clutch target preserves invariants");
}
