use cardbench_magic_engine::Keyword;
use cardbench_magic_rav::card_definitions;

#[test]
fn goblin_spelunkers_declares_mountainwalk() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOBLIN-SPELUNKERS")
        .expect("Goblin Spelunkers exists");
    assert!(definition.keywords.contains(&Keyword::Mountainwalk));
}
