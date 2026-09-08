//! Frozen printed-characteristic regressions, checked against Scryfall RAV records.
use cardbench_magic_engine::{Color, HybridManaSymbol, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn rav_printed_costs_colors_and_numeric_stats() {
    let cards = card_definitions();
    for (id, kind) in [("RAV-BLOCKBUSTER", cardbench_magic_engine::CardType::Enchantment),
        ("RAV-CLEANSING-BEAM", cardbench_magic_engine::CardType::Instant),
        ("RAV-DRYADS-CARESS", cardbench_magic_engine::CardType::Instant)] {
        let card = cards.iter().find(|card| card.id == id).unwrap();
        assert_eq!(card.card_types, [kind].into_iter().collect(), "{id} card type");
    }
    let card = cards.iter().find(|card| card.id == "RAV-BLOCKBUSTER").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(3, [Color::Red, Color::Red]), "Blockbuster mana_cost");
    assert_eq!(card.colors, [Color::Red].into_iter().collect(), "Blockbuster colors");
    let card = cards.iter().find(|card| card.id == "RAV-BOROS-GUILDMAGE").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_hybrid(0, [], [HybridManaSymbol { first: Color::Red, second: Color::White }, HybridManaSymbol { first: Color::Red, second: Color::White }]), "Boros Guildmage mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-CENTAUR-SAFEGUARD").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_hybrid(2, [], [HybridManaSymbol { first: Color::Green, second: Color::White }]), "Centaur Safeguard mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-CHORD-OF-CALLING").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(0, [Color::Green, Color::Green, Color::Green]), "Chord of Calling mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-CONCERTED-EFFORT").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(2, [Color::White, Color::White]), "Concerted Effort mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-DARK-HEART-OF-THE-WOOD").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(0, [Color::Black, Color::Green]), "Dark Heart of the Wood mana_cost");
    assert_eq!(card.colors, [Color::Black, Color::Green].into_iter().collect(), "Dark Heart of the Wood colors");
    let card = cards.iter().find(|card| card.id == "RAV-DOWSING-SHAMAN").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(4, [Color::Green]), "Dowsing Shaman mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-DRYADS-CARESS").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(4, [Color::Green, Color::Green]), "Dryad's Caress mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-FLASH-CONSCRIPTION").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(5, [Color::Red]), "Flash Conscription mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-GLEANCRAWLER").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_hybrid(3, [], [HybridManaSymbol { first: Color::Black, second: Color::Green }, HybridManaSymbol { first: Color::Black, second: Color::Green }, HybridManaSymbol { first: Color::Black, second: Color::Green }]), "Gleancrawler mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-HALCYON-GLAZE").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(1, [Color::Blue, Color::Blue]), "Halcyon Glaze mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-INDUCE-PARANOIA").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(2, [Color::Blue, Color::Blue]), "Induce Paranoia mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-INFECTIOUS-HOST").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(2, [Color::Black]), "Infectious Host mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-LAST-GASP").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(1, [Color::Black]), "Last Gasp mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-LIFE-FROM-THE-LOAM").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(1, [Color::Green]), "Life from the Loam mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-MINDLEECH-MASS").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(5, [Color::Blue, Color::Black, Color::Black]), "Mindleech Mass mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-MINDMOIL").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(4, [Color::Red]), "Mindmoil mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-NULLSTONE-GARGOYLE").unwrap();
    assert_eq!(card.mana_cost, ManaCost::new(9), "Nullstone Gargoyle mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-PERPLEX").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(1, [Color::Blue, Color::Black]), "Perplex mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-PLAGUE-BOILER").unwrap();
    assert_eq!(card.mana_cost, ManaCost::new(3), "Plague Boiler mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-POLLENBRIGHT-WINGS").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(4, [Color::Green, Color::White]), "Pollenbright Wings mana_cost");
    assert_eq!(card.colors, [Color::Green, Color::White].into_iter().collect(), "Pollenbright Wings colors");
    let card = cards.iter().find(|card| card.id == "RAV-PUTREFY").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(1, [Color::Black, Color::Green]), "Putrefy mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-QUICKCHANGE").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(1, [Color::Blue]), "Quickchange mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-REROUTE").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(1, [Color::Red]), "Reroute mana_cost");
    assert_eq!(card.colors, [Color::Red].into_iter().collect(), "Reroute colors");
    let card = cards.iter().find(|card| card.id == "RAV-ROOT-KIN-ALLY").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(4, [Color::Green, Color::Green]), "Root-Kin Ally mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-SABERTOOTH-ALLEY-CAT").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(1, [Color::Red, Color::Red]), "Sabertooth Alley Cat mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-SCION-OF-THE-WILD").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(1, [Color::Green, Color::Green]), "Scion of the Wild mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-SEISMIC-SPIKE").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(2, [Color::Red, Color::Red]), "Seismic Spike mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-SHADOW-OF-DOUBT").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_hybrid(0, [], [HybridManaSymbol { first: Color::Blue, second: Color::Black }, HybridManaSymbol { first: Color::Blue, second: Color::Black }]), "Shadow of Doubt mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-SISTERS-OF-STONE-DEATH").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(4, [Color::Black, Color::Black, Color::Green, Color::Green]), "Sisters of Stone Death mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-SMASH").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(2, [Color::Red]), "Smash mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-STASIS-CELL").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(4, [Color::Blue]), "Stasis Cell mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-TIDEWATER-MINION").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(3, [Color::Blue, Color::Blue]), "Tidewater Minion mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-VIASHINO-FANGTAIL").unwrap();
    assert_eq!(card.mana_cost, ManaCost::with_colors(2, [Color::Red, Color::Red]), "Viashino Fangtail mana_cost");
    let card = cards.iter().find(|card| card.id == "RAV-WOJEK-EMBERMAGE").unwrap();
    assert_eq!(card.toughness, Some(2), "Wojek Embermage toughness");
    let card = cards.iter().find(|card| card.id == "RAV-WOODWRAITH-STRANGLER").unwrap();
    assert_eq!(card.power, Some(2), "Woodwraith Strangler power");
}
