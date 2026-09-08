//! Printed Dryad's Caress counts battlefield creatures and conditionally untaps.
use cardbench_magic_engine::{CastRequest, Color, Game, ManaPaymentSelection, PlayerId, Zone};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn dryads_caress_requires_battlefield_count_and_white_spend_untap() {
    let definition = card_definitions().into_iter().find(|card| card.id == "RAV-DRYADS-CARESS").unwrap();
    assert!(definition.supported_rules.contains(&"battlefield-creature-life-gain-and-white-spend-untap"));
    assert_eq!(definition.effects.len(), 2);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}

#[test]
fn dryads_caress_counts_both_seats_and_untaps_only_own_creatures_if_white_was_spent() {
    for white in [false, true] {
        let mut game = Game::new(card_definitions(), 2).unwrap();
        let controller = PlayerId(0);
        let mine = game.put_on_battlefield(controller, "RAV-WATCHWOLF").unwrap();
        let theirs = game.put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF").unwrap();
        let land = game.put_on_battlefield(controller, "RAV-FOREST").unwrap();
        let grave = game.add_card(controller, "RAV-WATCHWOLF", Zone::Graveyard).unwrap();
        for card in [mine, theirs, land] { game.set_tapped_for_setup(card, true).unwrap(); }
        let spell = game.add_card(controller, "RAV-DRYADS-CARESS", Zone::Hand).unwrap();
        game.grant_mana(controller, Color::Green, if white { 5 } else { 6 }).unwrap();
        if white { game.grant_mana(controller, Color::White, 1).unwrap(); }
        let mut generic = vec![Color::Green; 3];
        generic.push(if white { Color::White } else { Color::Green });
        game.cast_spell_with_mana_spend(controller, CastRequest { card: spell,
            targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
            ManaPaymentSelection { generic, hybrid: vec![] }).unwrap();
        game.pass_priority(controller).unwrap();
        game.pass_priority(PlayerId(1)).unwrap();
        assert_eq!(game.player(controller).unwrap().life, 22);
        assert_eq!(game.object(mine).unwrap().tapped, !white);
        assert!(game.object(theirs).unwrap().tapped);
        assert!(game.object(land).unwrap().tapped);
        assert_eq!(game.zone_of(grave), Some(Zone::Graveyard));
        assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
        game.validate_invariants().unwrap();
    }
}
