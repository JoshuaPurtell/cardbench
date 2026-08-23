use cardbench_magic_engine::{Game, Keyword, PlayerId};
use cardbench_magic_rav::card_definitions;

#[test]
fn siege_wurm_exposes_its_printed_trample_keyword() {
    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let wurm = game
        .put_on_battlefield(PlayerId(0), "RAV-SIEGE-WURM")
        .expect("Siege Wurm exists");
    let characteristics = game.characteristics(wurm).expect("characteristics");
    assert!(characteristics.keywords.contains(&Keyword::Convoke));
    assert!(characteristics.keywords.contains(&Keyword::Trample));
}
