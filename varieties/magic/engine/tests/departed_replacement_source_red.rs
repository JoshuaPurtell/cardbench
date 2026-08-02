//! Red regression: persistent replacement effects retain historical spell
//! provenance when the spell's owner later leaves a multiplayer game.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const SHIELD: &str = "DEPARTED-REPLACEMENT-SOURCE-SHIELD";
const KILLER: &str = "DEPARTED-REPLACEMENT-SOURCE-KILLER";
const PING: &str = "DEPARTED-REPLACEMENT-SOURCE-PING";

fn instant(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["replacement-provenance-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The complete three-player replacement trace is intentionally explicit.
fn departed_spell_owner_does_not_invalidate_a_persistent_player_shield() {
    let survivor = PlayerId(0);
    let bystander = PlayerId(1);
    let departing_source_owner = PlayerId(2);
    let mut game = Game::new(
        [
            instant(
                SHIELD,
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
            ),
            instant(
                KILLER,
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
            instant(
                PING,
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
    )
    .expect("three-player fixture initializes");
    let shield = game
        .add_card(departing_source_owner, SHIELD, Zone::Hand)
        .expect("departing player holds the shield spell");
    let killer = game
        .add_card(survivor, KILLER, Zone::Hand)
        .expect("survivor holds lethal damage");
    let ping = game
        .add_card(survivor, PING, Zone::Hand)
        .expect("survivor holds a one-damage follow-up");

    game.pass_priority(survivor)
        .expect("first player passes to shield controller");
    game.pass_priority(bystander)
        .expect("bystander passes to shield controller");
    game.cast_spell(
        departing_source_owner,
        CastRequest {
            card: shield,
            targets: vec![Target::Player(survivor)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("source controller casts the persistent shield");
    game.pass_priority(departing_source_owner)
        .expect("source controller passes");
    game.pass_priority(survivor).expect("survivor passes");
    game.pass_priority(bystander)
        .expect("bystander resolves the shield");
    assert_eq!(game.zone_of(shield), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::DamageShieldCreated { source, target, amount }
            if *source == shield && *target == Target::Player(survivor) && *amount == 1)
    }));

    game.cast_spell(
        survivor,
        CastRequest {
            card: killer,
            targets: vec![Target::Player(departing_source_owner)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("survivor casts lethal damage at the shield source's owner");
    game.pass_priority(survivor).expect("survivor passes");
    game.pass_priority(bystander).expect("bystander passes");
    game.pass_priority(departing_source_owner)
        .expect("source owner leaving preserves historical shield provenance");

    assert!(game.players[departing_source_owner.0].lost);
    assert!(game.object(shield).is_err());
    let life_before_ping = game.players[survivor.0].life;
    game.cast_spell(
        survivor,
        CastRequest {
            card: ping,
            targets: vec![Target::Player(survivor)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("a surviving player may cast into the historical shield");
    game.pass_priority(survivor)
        .expect("caster passes the follow-up spell");
    game.pass_priority(bystander)
        .expect("bystander resolves the follow-up spell");
    assert_eq!(
        game.players[survivor.0].life, life_before_ping,
        "the one-shot shield still prevents its remaining point after source departure"
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::DamagePrevented { source, target, amount }
            if *source == ping && *target == Target::Player(survivor) && *amount == 1)
    }));
    game.validate_invariants()
        .expect("the player-targeted shield remains auditable after source removal");
}
