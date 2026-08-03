//! Red regression: combat damage to a permanent must use the same
//! affected-player replacement-order boundary as other prospective damage.
//!
//! A blocking creature is protected by both a bounded redirection and a
//! damage shield. Its controller chooses whether to redirect or prevent the
//! combat packet; the combat resolver must not silently use the legacy
//! redirect-first direct-damage path.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, CombatBlock, DecisionKind, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const ATTACKER: &str = "TST-COMBAT-PERMANENT-REPLACEMENT-ATTACKER";
const BLOCKER: &str = "TST-COMBAT-PERMANENT-REPLACEMENT-BLOCKER";
const REDIRECTOR: &str = "TST-COMBAT-PERMANENT-REPLACEMENT-REDIRECTOR";
const SHIELD: &str = "TST-COMBAT-PERMANENT-REPLACEMENT-SHIELD";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
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
        supported_rules: &["combat-permanent-damage-replacement-red"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_declare_attackers(game: &mut Game) {
    while game.step != cardbench_magic_engine::Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("normal turn priority advances");
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The priority transcript proves this is combat damage, not a direct spell.
fn blocker_controller_orders_redirection_and_shield_for_combat_damage() {
    let attacker_controller = PlayerId(0);
    let blocker_controller = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        [
            definition(
                ATTACKER,
                BTreeSet::from([CardType::Creature]),
                Some(2),
                Some(2),
                vec![],
            ),
            definition(
                BLOCKER,
                BTreeSet::from([CardType::Creature]),
                Some(0),
                Some(4),
                vec![],
            ),
            definition(
                REDIRECTOR,
                BTreeSet::from([CardType::Artifact]),
                None,
                None,
                vec![],
            ),
            definition(
                SHIELD,
                BTreeSet::from([CardType::Instant]),
                None,
                None,
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 2 }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: REDIRECTOR,
            ability: ActivatedAbility {
                id: "redirect-two",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![
                    TargetRequirement::Creature,
                    TargetRequirement::PlayerOrCreature,
                ],
                effects: vec![
                    Effect::BeginDamageRedirection { amount: 2 },
                    Effect::CompleteDamageRedirection,
                ],
            },
        }],
    )
    .expect("fixture constructs");
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker enters before game start");
    let blocker = game
        .put_on_battlefield(blocker_controller, BLOCKER)
        .expect("blocker enters before game start");
    let redirector = game
        .put_on_battlefield(blocker_controller, REDIRECTOR)
        .expect("redirector enters before game start");
    let shield = game
        .add_card(blocker_controller, SHIELD, Zone::Hand)
        .expect("shield begins in hand");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker predates the turn");
    game.begin_game().expect("game begins");

    advance_to_declare_attackers(&mut game);
    game.declare_attackers(attacker_controller, &[attacker])
        .expect("attacker declares");
    pass_pair(&mut game);
    game.declare_blockers(blocker_controller, &[CombatBlock { attacker, blocker }])
        .expect("blocker declares");

    game.pass_priority(attacker_controller)
        .expect("attacker controller passes in blocker window");
    game.cast_spell(
        blocker_controller,
        CastRequest {
            card: shield,
            targets: vec![Target::Permanent(blocker)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("blocker controller creates its shield");
    pass_pair(&mut game);
    game.pass_priority(attacker_controller)
        .expect("active player passes after shield resolution");
    game.activate_ability(
        blocker_controller,
        AbilityActivation {
            source: redirector,
            ability_id: "redirect-two",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![
                Target::Permanent(blocker),
                Target::Player(attacker_controller),
            ],
        },
    )
    .expect("blocker controller creates its redirection");
    pass_pair(&mut game);

    // The next pair begins ordinary combat damage.  The two replacements are
    // concurrent for the blocker, so damage cannot be committed until its
    // controller submits a `Replacement` decision.
    pass_pair(&mut game);
    let view = game
        .view_for_player(blocker_controller)
        .expect("blocker controller view");
    eprintln!(
        "combat permanent replacement red trace: step={:?}; pending={:?}; blocker_damage={}; lives=({}, {}); events={:?}",
        game.step,
        view.pending_decision,
        game.object(blocker).expect("blocker exists").damage,
        game.player(attacker_controller)
            .expect("attacker player")
            .life,
        game.player(blocker_controller)
            .expect("blocker player")
            .life,
        game.canonical_event_log(),
    );
    let decision = view
        .pending_decision
        .expect("combat blocker damage must open a replacement decision");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(game.next_policy_player(), blocker_controller);
    assert_eq!(game.object(blocker).expect("blocker exists").damage, 0);
    assert_eq!(
        game.player(attacker_controller)
            .expect("attacker player")
            .life,
        20
    );
    game.validate_invariants()
        .expect("suspended combat permanent replacement is invariant-valid");
}
