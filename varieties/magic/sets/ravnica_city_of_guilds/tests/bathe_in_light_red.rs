//! Red discovery contract for Bathe in Light's policy-chosen protection color.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, PolicyAction,
    Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn bathe_in_light_has_its_chosen_color_team_protection_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BATHE-IN-LIGHT")
        .expect("Bathe in Light definition exists");
    assert_eq!(definition.name, "Bathe in Light");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"chosen-color-controller-creature-protection")
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
#[allow(clippy::too_many_lines)] // One trace proves choice provenance and controller-scoped protection.
fn bathe_in_light_retains_the_policy_color_and_protects_only_controller_creatures() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let protected_creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("colored friendly creature setup");
    let protected_colorless_creature = game
        .put_on_battlefield(PlayerId(0), "RAV-GLASS-GOLEM")
        .expect("colorless friendly creature setup");
    let opponent_creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent creature setup");
    let bathe = game
        .add_card(PlayerId(0), "RAV-BATHE-IN-LIGHT", Zone::Hand)
        .expect("Bathe in Light setup");
    let peel = game
        .add_card(PlayerId(1), "RAV-PEEL-FROM-REALITY", Zone::Hand)
        .expect("blue targeting spell setup");
    game.grant_mana(PlayerId(0), Color::White, 2)
        .expect("pre-game white payment setup");

    game.submit_policy_move(
        PlayerId(0),
        "rav-bathe-in-light-policy",
        PolicyAction::CastWithColorChoice {
            request: CastRequest {
                card: bathe,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            color: Color::Blue,
        },
    )
    .expect("policy submits Blue as the spell choice");
    pass_pair(&mut game);

    for creature in [protected_creature, protected_colorless_creature] {
        assert!(
            game.characteristics(creature)
                .expect("friendly creature remains visible")
                .keywords
                .contains(&Keyword::Protection(Color::Blue))
        );
    }
    assert!(
        !game
            .characteristics(opponent_creature)
            .expect("opponent creature remains visible")
            .keywords
            .contains(&Keyword::Protection(Color::Blue))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellColorChosen { player, card, color }
            if *player == PlayerId(0) && *card == bathe && *color == Color::Blue
    )));

    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: peel,
            targets: vec![
                Target::Permanent(opponent_creature),
                Target::Permanent(protected_creature),
            ],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect_err("the Blue spell cannot choose the protected opposing creature");
    assert_eq!(game.zone_of(peel), Some(Zone::Hand));
    eprintln!("Bathe in Light trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Bathe in Light preserves invariant state");
}
