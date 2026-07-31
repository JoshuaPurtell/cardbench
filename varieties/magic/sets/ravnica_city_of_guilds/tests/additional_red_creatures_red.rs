//! Red discovery probes for the next easy creature/ETB/static slice.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ObjectId, PlayerId, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};
use cardbench_magic_rav::{
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn sparkmage_apprentice_requires_targeted_etb_damage() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SPARKMAGE-APPRENTICE")
        .expect("Sparkmage Apprentice definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.supported_rules.contains(&"etb-targeted-damage"));
}

#[test]
fn hunted_dragon_requires_haste_flying_and_opponent_knight_etb() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HUNTED-DRAGON")
        .expect("Hunted Dragon definition exists");
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.keywords.contains(&Keyword::Flying));
    assert!(definition.keywords.contains(&Keyword::Haste));
    assert!(
        definition
            .supported_rules
            .contains(&"etb-targeted-opponent-knight-tokens")
    );
}

#[test]
fn razia_requires_flying_vigilance_and_haste() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-RAZIA-BOROS-ARCHANGEL")
        .expect("Razia, Boros Archangel definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.keywords.contains(&Keyword::Flying));
    assert!(definition.keywords.contains(&Keyword::Vigilance));
    assert!(definition.keywords.contains(&Keyword::Haste));
}

fn fixture_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn advance_to_first_main(game: &mut Game) {
    game.begin_game().expect("game starts");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player).expect("advance to first main");
    }
}

fn setup_mountains(game: &mut Game, count: usize) -> Vec<ObjectId> {
    (0..count)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain setup")
        })
        .collect()
}

fn activate_mountains(game: &mut Game, mountains: &[ObjectId]) {
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), *mountain, Color::Red)
            .expect("activate red Mountain");
    }
}

#[test]
fn sparkmage_apprentice_etb_targets_opponent_and_deals_one() {
    let mut game = fixture_game();
    let sparkmage = game
        .add_card(PlayerId(0), "RAV-SPARKMAGE-APPRENTICE", Zone::Hand)
        .expect("Sparkmage enters hand");
    let mountains = setup_mountains(&mut game, 2);
    advance_to_first_main(&mut game);
    activate_mountains(&mut game, &mountains);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: sparkmage,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast Sparkmage");
    game.pass_priority(PlayerId(0)).expect("cast priority pass");
    game.pass_priority(PlayerId(1)).expect("creature resolves");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == sparkmage && *ability == "etb-deal-one"
    )));
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");
    assert_eq!(game.player(PlayerId(1)).expect("opponent exists").life, 19);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { player, amount, .. }
            if *player == PlayerId(1) && *amount == 1
    )));
}

#[test]
fn hunted_dragon_etb_creates_three_first_strike_knights_for_one_targeted_opponent() {
    let mut game = fixture_game();
    let dragon = game
        .add_card(PlayerId(0), "RAV-HUNTED-DRAGON", Zone::Hand)
        .expect("Hunted Dragon enters hand");
    let mountains = setup_mountains(&mut game, 5);
    advance_to_first_main(&mut game);
    activate_mountains(&mut game, &mountains);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: dragon,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast Hunted Dragon");
    game.pass_priority(PlayerId(0)).expect("cast priority pass");
    game.pass_priority(PlayerId(1)).expect("dragon resolves");
    assert_eq!(
        game.stack.last().map(|object| object.targets.clone()),
        Some(vec![cardbench_magic_engine::Target::Player(PlayerId(1))])
    );
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("token trigger resolves");
    let opponent_battlefield = &game
        .player(PlayerId(1))
        .expect("opponent exists")
        .battlefield;
    assert_eq!(opponent_battlefield.len(), 3);
    for token in opponent_battlefield {
        let characteristics = game.characteristics(*token).expect("token characteristics");
        assert_eq!(characteristics.power, Some(2));
        assert_eq!(characteristics.toughness, Some(2));
        assert!(characteristics.keywords.contains(&Keyword::FirstStrike));
    }
}

#[test]
fn razia_redirects_the_next_three_damage_to_the_second_target() {
    let mut game = fixture_game();
    let razia = game
        .put_on_battlefield(PlayerId(0), "RAV-RAZIA-BOROS-ARCHANGEL")
        .expect("Razia enters");
    game.set_entered_turn_for_setup(razia, 0)
        .expect("Razia has prior-turn provenance");
    let char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("Char enters hand");
    let mountains = setup_mountains(&mut game, 3);
    advance_to_first_main(&mut game);
    game.activate_ability(
        PlayerId(0),
        cardbench_magic_engine::AbilityActivation {
            source: razia,
            ability_id: "tap-redirect-three-damage",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![
                cardbench_magic_engine::Target::Permanent(razia),
                cardbench_magic_engine::Target::Player(PlayerId(1)),
            ],
        },
    )
    .expect("activate Razia redirection");
    game.pass_priority(PlayerId(0))
        .expect("ability controller passes");
    game.pass_priority(PlayerId(1))
        .expect("redirection resolves");
    activate_mountains(&mut game, &mountains);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![cardbench_magic_engine::Target::Permanent(razia)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast Char at Razia");
    game.pass_priority(PlayerId(0))
        .expect("Char controller passes");
    game.pass_priority(PlayerId(1)).expect("Char resolves");
    assert_eq!(game.player(PlayerId(1)).expect("opponent exists").life, 17);
    assert_eq!(game.object(razia).expect("Razia exists").damage, 1);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageRedirected { from, to, amount, .. }
            if *from == razia
                && *to == cardbench_magic_engine::Target::Player(PlayerId(1))
                && *amount == 3
    )));
}
