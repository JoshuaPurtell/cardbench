//! Red regression: redirected combat damage that reaches a creature still
//! satisfies the source's combat-damage-to-a-creature trigger condition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CombatBlock, Effect, Game, GameEvent, ManaCost, PlayerId, Target, TargetRequirement,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

const ATTACKER: &str = "TST-COMBAT-REDIRECT-TRIGGER-ATTACKER";
const BLOCKER: &str = "TST-COMBAT-REDIRECT-TRIGGER-BLOCKER";
const DESTINATION: &str = "TST-COMBAT-REDIRECT-TRIGGER-DESTINATION";
const REDIRECTOR: &str = "TST-COMBAT-REDIRECT-TRIGGER-REDIRECTOR";
const COMBAT_TRIGGER: &str = "combat-hit";

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
        supported_rules: &["combat-redirected-creature-trigger-red"],
        power,
        toughness,
        keywords: vec![],
        effects: vec![],
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
fn redirected_combat_damage_to_another_creature_stacks_the_source_trigger() {
    let attacker_controller = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                ATTACKER,
                BTreeSet::from([CardType::Creature]),
                Some(2),
                Some(2),
            ),
            definition(
                BLOCKER,
                BTreeSet::from([CardType::Creature]),
                Some(0),
                Some(4),
            ),
            definition(
                DESTINATION,
                BTreeSet::from([CardType::Creature]),
                Some(0),
                Some(4),
            ),
            definition(REDIRECTOR, BTreeSet::from([CardType::Artifact]), None, None),
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
        [TriggeredAbilityBinding {
            card_definition: ATTACKER,
            ability: TriggeredAbility {
                id: COMBAT_TRIGGER,
                condition: TriggerCondition::DealsCombatDamageToCreature,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::DestroyCombatDamagedCreature],
            },
        }],
    )
    .expect("fixture constructs");
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker enters before game start");
    let blocker = game
        .put_on_battlefield(defender, BLOCKER)
        .expect("blocker enters before game start");
    let destination = game
        .put_on_battlefield(defender, DESTINATION)
        .expect("redirect destination enters before game start");
    let redirector = game
        .put_on_battlefield(defender, REDIRECTOR)
        .expect("redirector enters before game start");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker predates the turn");
    game.begin_game().expect("game begins");

    advance_to_declare_attackers(&mut game);
    game.declare_attackers(attacker_controller, &[attacker])
        .expect("attacker declares");
    pass_pair(&mut game);
    game.declare_blockers(defender, &[CombatBlock { attacker, blocker }])
        .expect("blocker declares");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes in blocker window");
    game.activate_ability(
        defender,
        AbilityActivation {
            source: redirector,
            ability_id: "redirect-two",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(blocker), Target::Permanent(destination)],
        },
    )
    .expect("redirector activates");
    pass_pair(&mut game);
    pass_pair(&mut game);

    eprintln!(
        "redirected combat trigger red trace: destination_damage={}; stack={:?}; events={:?}",
        game.object(destination).expect("destination exists").damage,
        game.stack,
        game.canonical_event_log(),
    );
    assert_eq!(
        game.object(destination).expect("destination exists").damage,
        2
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                controller,
                source,
                ability: COMBAT_TRIGGER,
                ..
            } if *controller == attacker_controller && *source == attacker
        )),
        "the final creature recipient must preserve combat-damage trigger provenance"
    );
    game.validate_invariants()
        .expect("redirected combat trigger state remains valid");
}
