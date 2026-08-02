//! Red regression: CR 800.4a owner removal must clear an effect-created cast
//! permission whose authorized card is removed from exile with its owner.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastPermissionPayment, CastRequest, CastTiming, Effect, Game,
    GameEvent, ManaCost, PlayerId, Target, TargetRequirement, Zone,
};

const PERMISSION: &str = "DEPARTED-CAST-PERMISSION-GRANT";
const EXILED_SPELL: &str = "DEPARTED-CAST-PERMISSION-SPELL";
const KILLER: &str = "DEPARTED-CAST-PERMISSION-KILLER";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["departed-cast-permission-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn departing_owner_clears_its_effect_created_exile_cast_permission() {
    let survivor = PlayerId(0);
    let bystander = PlayerId(1);
    let departing_permission_controller = PlayerId(2);
    let mut game = Game::new(
        [
            definition(
                PERMISSION,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::GrantExileCastPermissionUntilEndOfTurn {
                    payment: CastPermissionPayment::WithoutPayingManaCost,
                    timing: CastTiming::AsThoughInstant,
                }],
            ),
            definition(EXILED_SPELL, BTreeSet::from([CardType::Sorcery]), vec![]),
            definition(
                KILLER,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
    )
    .expect("three-player fixture initializes");
    let permission = game
        .add_card(departing_permission_controller, PERMISSION, Zone::Hand)
        .expect("departing player holds the permission spell");
    let exiled_spell = game
        .add_card(departing_permission_controller, EXILED_SPELL, Zone::Exile)
        .expect("departing player starts with an exiled spell");
    let killer = game
        .add_card(survivor, KILLER, Zone::Hand)
        .expect("survivor holds lethal damage");

    game.pass_priority(survivor)
        .expect("survivor passes to the bystander");
    game.pass_priority(bystander)
        .expect("bystander passes to permission controller");
    game.cast_spell(
        departing_permission_controller,
        CastRequest {
            card: permission,
            targets: vec![Target::Permanent(exiled_spell)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("permission controller grants an exile cast permission");
    game.pass_priority(departing_permission_controller)
        .expect("permission controller passes");
    game.pass_priority(survivor).expect("survivor passes");
    game.pass_priority(bystander)
        .expect("bystander resolves the permission spell");
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::CastPermissionGranted { card, .. } if *card == exiled_spell)
    }));

    game.cast_spell(
        survivor,
        CastRequest {
            card: killer,
            targets: vec![Target::Player(departing_permission_controller)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("survivor casts lethal damage at the permission controller");
    game.pass_priority(survivor).expect("survivor passes");
    game.pass_priority(bystander).expect("bystander passes");
    game.pass_priority(departing_permission_controller)
        .expect("departure clears the owned exile cast permission");

    assert!(game.players[departing_permission_controller.0].lost);
    assert!(game.object(exiled_spell).is_err());
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::CastPermissionExpired { card, from: Zone::Exile }
            if *card == exiled_spell)
    }));
    game.validate_invariants()
        .expect("no departed card retains a live effect-created cast permission");
}
