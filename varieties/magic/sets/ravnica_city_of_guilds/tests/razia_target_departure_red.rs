//! Red regression: Razia's two-target replacement must not error when the
//! protected creature leaves before the ability resolves.

use cardbench_magic_engine::{AbilityActivation, CastRequest, Color, Game, PlayerId, Target, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn razia_target_departure_does_not_leak_a_failed_resolution() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let razia = game
        .put_on_battlefield(PlayerId(0), "RAV-RAZIA-BOROS-ARCHANGEL")
        .expect("Razia enters");
    let recruit = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-RECRUIT")
        .expect("protected creature enters");
    game.set_entered_turn_for_setup(razia, 0)
        .expect("Razia has prior-turn provenance");
    game.set_entered_turn_for_setup(recruit, 0)
        .expect("Recruit has prior-turn provenance");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("Char enters opponent hand");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
                .expect("opponent red mana source enters before game start")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player).expect("advance to first main");
    }
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: razia,
            ability_id: "tap-redirect-three-damage",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(recruit), Target::Player(PlayerId(1))],
        },
    )
    .expect("Razia ability activates");
    game.pass_priority(PlayerId(0))
        .expect("Razia controller passes");
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(1), mountain, Color::Red)
            .expect("activate opponent red mana source");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(recruit)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent responds with Char");
    game.pass_priority(PlayerId(1)).expect("Char caster passes");
    game.pass_priority(PlayerId(0))
        .expect("Char resolves and removes target");
    game.pass_priority(PlayerId(0))
        .expect("Razia controller passes after target departure");
    game.pass_priority(PlayerId(1))
        .expect("Razia ability resolves without leaking an error");
    assert!(
        game.player(PlayerId(0))
            .expect("controller exists")
            .battlefield
            .contains(&razia)
    );
}
