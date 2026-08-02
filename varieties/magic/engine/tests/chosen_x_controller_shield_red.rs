//! Red regression for an X-valued player damage-prevention shield.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost,
    ManaPaymentSelection, PlayerId, Target, TargetRequirement, Zone,
};

const FESTIVAL: &str = "TST-CHOSEN-X-CONTROLLER-SHIELD";
const DRAW_CARD: &str = "TST-CHOSEN-X-DRAW-CARD";
const DAMAGE: &str = "TST-CHOSEN-X-DAMAGE";

fn definition(
    id: &'static str,
    card_type: CardType,
    mana_cost: ManaCost,
    effects: Vec<Effect>,
) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["chosen-x-controller-shield-contract"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

#[test]
fn chosen_x_controller_shield_survives_spell_resolution_and_caps_later_damage() {
    let mut game = Game::new(
        vec![
            definition(
                FESTIVAL,
                CardType::Instant,
                ManaCost::with_colors(0, [Color::White]),
                vec![
                    Effect::AddControllerDamageShieldEqualToChosenXUntilEndOfTurn,
                    Effect::DrawController,
                ],
            ),
            definition(DRAW_CARD, CardType::Creature, ManaCost::new(0), vec![]),
            definition(
                DAMAGE,
                CardType::Instant,
                ManaCost::new(0),
                vec![Effect::DealDamage {
                    amount: 4,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        2,
    )
    .expect("synthetic fixture builds");
    let festival = game
        .add_card(PlayerId(0), FESTIVAL, Zone::Hand)
        .expect("shield spell setup");
    let drawn = game
        .add_card(PlayerId(0), DRAW_CARD, Zone::Library)
        .expect("draw-card setup");
    let damage = game
        .add_card(PlayerId(1), DAMAGE, Zone::Hand)
        .expect("damage spell setup");
    game.grant_mana(PlayerId(0), Color::White, 3)
        .expect("X=2 plus white payment exists");

    game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: festival,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        2,
        ManaPaymentSelection {
            generic: vec![Color::White, Color::White],
            hybrid: vec![],
        },
    )
    .expect("chosen-X shield spell casts");
    game.pass_priority(PlayerId(0))
        .expect("controller passes shield spell");
    game.pass_priority(PlayerId(1))
        .expect("shield spell resolves");
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageShieldCreated { source, target, amount }
            if *source == festival && *target == Target::Player(PlayerId(0)) && *amount == 2
    )));

    game.pass_priority(PlayerId(0))
        .expect("damage controller receives priority");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: damage,
            targets: vec![Target::Player(PlayerId(0))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("later damage spell casts");
    game.pass_priority(PlayerId(1))
        .expect("damage controller passes");
    game.pass_priority(PlayerId(0))
        .expect("damage spell resolves");
    assert_eq!(game.players[0].life, 18);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented { source, target, amount }
            if *source == damage && *target == Target::Player(PlayerId(0)) && *amount == 2
    )));
    game.validate_invariants()
        .expect("chosen-X controller shield is replay-valid");
}
