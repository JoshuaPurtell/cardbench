use cardbench_magic_engine::{CardType, Game, Keyword};
use cardbench_magic_rav::card_definitions;

#[test]
fn autochthon_wurm_exposes_its_printed_trample_keyword() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-AUTOCHTHON-WURM")
        .expect("Autochthon Wurm is catalogued");
    assert_eq!(definition.card_types, [CardType::Creature].into_iter().collect());
    assert!(definition.keywords.contains(&Keyword::Convoke));
    assert!(definition.keywords.contains(&Keyword::Trample));
    assert!(definition.supported_rules.contains(&"trample"));
    let _ = Game::new(card_definitions(), 2).expect("RAV catalog validates");
}
