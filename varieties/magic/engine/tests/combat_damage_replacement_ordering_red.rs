//! Red regression: an affected player must order concurrent combat-damage
//! replacements before either replacement commits its result.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, DamageReplacementEffect, DamageReplacementEffectBinding, Game,
    ManaCost, PlayerId, Step, Zone,
};

const REPLACER: &str = "COMBAT-REPLACEMENT-SOURCE";
const HALVER: &str = "COMBAT-REPLACEMENT-HALVER";
const LIBRARY_CARD: &str = "COMBAT-REPLACEMENT-LIBRARY";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
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
        supported_rules: &["combat-replacement-ordering-probe"],
        power,
        toughness,
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_to(game: &mut Game, step: Step) {
    while game.step != step {
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("priority advances the deterministic combat fixture");
    }
}

#[test]
fn affected_player_orders_source_combat_and_global_damage_replacements() {
    let attacker_controller = PlayerId(0);
    let affected_player = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                REPLACER,
                BTreeSet::from([CardType::Creature]),
                Some(4),
                Some(4),
            ),
            definition(HALVER, BTreeSet::from([CardType::Enchantment]), None, None),
            definition(
                LIBRARY_CARD,
                BTreeSet::from([CardType::Artifact]),
                None,
                None,
            ),
        ],
        2,
    )
    .expect("two-player fixture initializes");
    game.register_damage_replacement_effect_bindings([
        DamageReplacementEffectBinding {
            source_definition: REPLACER,
            effect: DamageReplacementEffect::ReplaceCombatDamageToPlayerWithMillAndCounters,
        },
        DamageReplacementEffectBinding {
            source_definition: HALVER,
            effect: DamageReplacementEffect::HalveDamage,
        },
    ])
    .expect("competing replacement bindings register before game start");
    let attacker = game
        .put_on_battlefield(attacker_controller, REPLACER)
        .expect("attacker begins on the battlefield");
    game.put_on_battlefield(affected_player, HALVER)
        .expect("global halver begins on the battlefield");
    for _ in 0..4 {
        game.add_card(affected_player, LIBRARY_CARD, Zone::Library)
            .expect("affected player has a four-card library");
    }
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker predates this turn");
    game.begin_game().expect("fixture starts");
    pass_to(&mut game, Step::DeclareAttackers);
    game.declare_attackers(attacker_controller, &[attacker])
        .expect("attacker attacks");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes");
    game.pass_priority(affected_player)
        .expect("fixture advances to blockers");
    game.declare_blockers(affected_player, &[])
        .expect("affected player declares no blockers");

    // First-strike damage has no assignments. The following two passes begin
    // ordinary combat damage, where both replacements apply to the same
    // prospective four-damage player packet.
    for _ in 0..4 {
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("combat advances to the replacement boundary");
    }

    let decision = game
        .view_for_player(affected_player)
        .expect("affected player has a view")
        .pending_decision
        .expect("affected player must order concurrent combat replacements");
    assert_eq!(
        game.next_policy_player(),
        affected_player,
        "the affected player owns the replacement choice",
    );
    assert_eq!(
        decision.kind,
        cardbench_magic_engine::DecisionKind::Replacement,
        "replacement ordering is a typed no-priority decision",
    );
    assert_eq!(
        game.player(affected_player)
            .expect("affected player remains live")
            .library
            .len(),
        4,
        "neither replacement commits before the affected player chooses an order",
    );
    game.validate_invariants()
        .expect("the suspended combat replacement state is auditable");
}
