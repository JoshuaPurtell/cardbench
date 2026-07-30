//! Red regression: a damage effect cannot create negative marked damage.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, PolicyAction,
    Target, TargetRequirement, Zone,
};

const NEGATIVE_DAMAGE_SPELL: &str = "TST-NEGATIVE-DAMAGE";
const CREATURE: &str = "TST-NEGATIVE-DAMAGE-TARGET";

fn definitions() -> [CardDefinition; 2] {
    [
        CardDefinition {
            id: NEGATIVE_DAMAGE_SPELL,
            name: "negative damage spell",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: -1,
                target: TargetRequirement::Creature,
            }],
        },
        CardDefinition {
            id: CREATURE,
            name: "negative damage target",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
    ]
}

#[test]
fn a_negative_damage_definition_cannot_leave_a_rejected_policy_move_with_negative_damage() {
    let mut game = Game::new(definitions(), 2).expect("two-player game initializes");
    let spell = game
        .add_card(PlayerId(0), NEGATIVE_DAMAGE_SPELL, Zone::Hand)
        .expect("spell enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("target starts on the battlefield");
    game.clear_event_log();

    let cast = game.submit_policy_move(
        PlayerId(0),
        "red-regression",
        PolicyAction::Cast(CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
        }),
    );
    // A corrected engine may reject the malformed executable definition at
    // cast time. That is an acceptable fail-closed outcome.
    if cast.is_err() {
        assert_eq!(game.zone_of(spell), Some(Zone::Hand));
        assert_eq!(game.object(target).expect("target remains").damage, 0);
        return;
    }

    game.submit_policy_move(PlayerId(0), "red-regression", PolicyAction::PassPriority)
        .expect("caster can pass priority");
    let before_resolution = game.canonical_event_log();
    let result = game.submit_policy_move(PlayerId(1), "red-regression", PolicyAction::PassPriority);

    assert!(
        result.is_err(),
        "the malformed spell must not resolve successfully"
    );
    assert_eq!(
        game.object(target).expect("target remains").damage,
        0,
        "a rejected pass mutated marked damage; before: {before_resolution:?}; after: {:?}",
        game.canonical_event_log(),
    );
}
