//! Red regression: player departure must not strand LKI for removed objects.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const SELF_DEFEAT: &str = "TST-LKI-OWNER-DEPARTURE";

#[test]
fn player_departure_removes_last_known_characteristics_of_owned_cards() {
    let definition = CardDefinition {
        id: SELF_DEFEAT,
        name: SELF_DEFEAT,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["last-known-owner-departure-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DealDamage {
            amount: 20,
            target: TargetRequirement::Player,
        }],
    };
    let mut game = Game::new([definition], 2).expect("fixture initializes");
    let spell = game
        .add_card(PlayerId(0), SELF_DEFEAT, Zone::Hand)
        .expect("spell enters hand");
    game.begin_game().expect("game begins");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Player(PlayerId(0))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    let result = game.pass_priority(PlayerId(1));
    eprintln!("owner departure LKI red result={result:?}; events={:?}", game.canonical_event_log());
    result.expect("player loss must remove all private LKI for their removed objects");
    assert!(game.players[PlayerId(0).0].lost);
    game.validate_invariants().expect("departure leaves no orphan LKI");
}
