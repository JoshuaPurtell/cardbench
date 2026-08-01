//! Public compatibility contract for Hunted Phantasm's represented ETB slice.

use cardbench_magic_engine::{
    CastRequest, Color, CreatureSubtype, Game, GameEvent, Keyword, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn hunted_phantasm_is_unblockable_and_creates_five_red_goblins_for_an_opponent() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HUNTED-PHANTASM")
        .expect("Hunted Phantasm definition exists");
    assert!(definition.keywords.contains(&Keyword::Unblockable));

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
    let phantasm = game
        .add_card(PlayerId(0), "RAV-HUNTED-PHANTASM", Zone::Hand)
        .expect("Hunted Phantasm is executable");
    let islands = [
        game.add_card(PlayerId(0), "RAV-ISLAND", Zone::Battlefield)
            .expect("first Island enters"),
        game.add_card(PlayerId(0), "RAV-ISLAND", Zone::Battlefield)
            .expect("second Island enters"),
        game.add_card(PlayerId(0), "RAV-ISLAND", Zone::Battlefield)
            .expect("third Island enters"),
    ];

    game.begin_game().expect("fixture starts game");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward main phase");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward main phase");
    }
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("Island produces blue mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: phantasm,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Hunted Phantasm casts without a spell target");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the creature spell");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes the creature spell");
    assert_eq!(
        game.stack.last().map(|object| object.targets.clone()),
        Some(vec![Target::Player(PlayerId(1))]),
        "the ETB trigger deterministically selects one opponent"
    );
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("token trigger resolves");

    println!("Hunted Phantasm trace: {:?}", game.canonical_event_log());
    let tokens = &game
        .player(PlayerId(1))
        .expect("opponent exists")
        .battlefield;
    assert_eq!(tokens.len(), 5, "exactly five Goblins are created");
    for token in tokens {
        let characteristics = game.characteristics(*token).expect("token characteristics");
        assert_eq!(characteristics.colors, [Color::Red].into_iter().collect());
        assert_eq!(characteristics.power, Some(1));
        assert_eq!(characteristics.toughness, Some(1));
        assert!(
            characteristics
                .creature_subtypes
                .contains(&CreatureSubtype::Goblin)
        );
    }
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::TokenCreated { player, .. } if *player == PlayerId(1)))
            .count(),
        5
    );
    game.validate_invariants()
        .expect("Hunted Phantasm ETB preserves invariants");
}
