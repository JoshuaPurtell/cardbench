//! Red regression for Brainspoil's source-lane front-face omission.

use cardbench_magic_engine::{CastRequest, Color, Effect, Game, PlayerId, Step, Target, Zone};
use cardbench_magic_rav::card_definitions;

fn game() -> Game {
    Game::new(card_definitions(), 2).expect("RAV game builds")
}

fn add_payment(game: &mut Game) {
    game.grant_mana(PlayerId(0), Color::Black, 2)
        .expect("black mana");
    game.grant_mana(PlayerId(0), Color::Blue, 3)
        .expect("generic mana");
}

#[test]
fn brainspoil_destroys_a_nonblack_creature_and_rejects_a_black_one_atomically() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BRAINSPOIL")
        .expect("Brainspoil definition exists");
    assert_eq!(
        definition.effects,
        vec![Effect::DestroyTargetNonblackCreature],
        "Brainspoil must expose its typed nonblack creature-destruction face"
    );

    let mut legal = game();
    legal.step = Step::PrecombatMain;
    let target = legal
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("nonblack target");
    let brainspoil = legal
        .add_card(PlayerId(0), "RAV-BRAINSPOIL", Zone::Hand)
        .expect("Brainspoil in hand");
    add_payment(&mut legal);
    legal
        .cast_spell(
            PlayerId(0),
            CastRequest {
                card: brainspoil,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect("Brainspoil accepts a nonblack creature");
    legal.pass_priority(PlayerId(0)).expect("caster passes");
    legal.pass_priority(PlayerId(1)).expect("opponent passes");
    println!("Brainspoil nonblack trace: {:?}", legal.event_log);
    assert_eq!(legal.zone_of(target), Some(Zone::Graveyard));
    legal
        .validate_invariants()
        .expect("legal Brainspoil trace preserves invariants");

    let mut illegal = game();
    illegal.step = Step::PrecombatMain;
    let black_target = illegal
        .put_on_battlefield(PlayerId(1), "RAV-SEWERDREG")
        .expect("black creature target");
    let brainspoil = illegal
        .add_card(PlayerId(0), "RAV-BRAINSPOIL", Zone::Hand)
        .expect("Brainspoil in hand");
    add_payment(&mut illegal);
    let before_events = illegal.event_log.clone();
    assert!(
        illegal
            .cast_spell(
                PlayerId(0),
                CastRequest {
                    card: brainspoil,
                    targets: vec![Target::Permanent(black_target)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
            )
            .is_err(),
        "Brainspoil must reject a black creature before costs or stack mutation"
    );
    assert_eq!(illegal.zone_of(brainspoil), Some(Zone::Hand));
    assert_eq!(illegal.zone_of(black_target), Some(Zone::Battlefield));
    assert_eq!(illegal.event_log, before_events);
    illegal
        .validate_invariants()
        .expect("rejected Brainspoil target preserves invariants");
}
