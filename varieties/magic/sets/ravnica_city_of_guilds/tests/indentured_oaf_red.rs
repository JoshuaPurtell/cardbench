//! Red regression for Indentured Oaf's source-bound damage prevention.

use cardbench_magic_engine::{
    CardType, Color, CombatBlock, Game, GameEvent, Keyword, PlayerId, Step, Target,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn indentured_oaf_prevents_its_damage_to_red_creatures() {
    let oaf_definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-INDENTURED-OAF")
        .expect("Indentured Oaf definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&oaf_definition.id));
    assert_eq!(
        oaf_definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!(oaf_definition.colors, [Color::Red].into_iter().collect());
    assert!(
        oaf_definition
            .supported_rules
            .contains(&"prevent-damage-by-this-source-to-red-creatures")
    );
    assert!(
        oaf_definition
            .keywords
            .contains(&Keyword::PreventDamageToCreaturesOfColor(Color::Red))
    );

    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let oaf = game
        .put_on_battlefield(PlayerId(0), "RAV-INDENTURED-OAF")
        .expect("Oaf enters");
    let red_blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-GOBLIN-FIRE-FIEND")
        .expect("red creature enters");
    for creature in [oaf, red_blocker] {
        game.set_entered_turn_for_setup(creature, 0)
            .expect("creature predates the measured turn");
    }
    game.begin_game().expect("game starts");
    while game.step != Step::DeclareAttackers {
        let player = game.priority;
        game.pass_priority(player).expect("advance to attackers");
    }
    game.declare_attackers(PlayerId(0), &[oaf])
        .expect("Oaf attacks");
    while game.step != Step::DeclareBlockers {
        let player = game.priority;
        game.pass_priority(player).expect("advance to blockers");
    }
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: oaf,
            blocker: red_blocker,
        }],
    )
    .expect("red Goblin blocks");

    for _ in 0..16 {
        if game.event_log.iter().any(|event| {
            matches!(
                event,
                GameEvent::DamagePrevented {
                    source,
                    target: Target::Permanent(target),
                    amount: 4,
                } if *source == oaf && *target == red_blocker
            )
        }) {
            break;
        }
        let player = game.priority;
        game.pass_priority(player).expect("combat advances");
    }

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented {
            source,
            target: Target::Permanent(target),
            amount: 4,
        } if *source == oaf && *target == red_blocker
    )));
    assert_eq!(
        game.object(red_blocker).expect("red blocker exists").damage,
        0
    );
    assert_eq!(game.object(oaf).expect("Oaf remains").damage, 1);
    game.validate_invariants()
        .expect("Oaf's source-bound prevention trace is valid");
}
