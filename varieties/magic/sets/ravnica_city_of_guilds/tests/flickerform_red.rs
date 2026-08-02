//! Red discovery contract for Flickerform's Aura-linked blink activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Step,
    Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn rav_game() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn advance_to_main(game: &mut Game) {
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first player passes");
        let second = game.priority;
        game.pass_priority(second).expect("second player passes");
    }
    assert_eq!(game.step, Step::PrecombatMain);
}

fn advance_until_delayed_return(game: &mut Game) {
    for _ in 0..32 {
        if game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::DelayedActionConsumed { .. }))
        {
            return;
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .event_log
                    .iter()
                    .any(|event| matches!(event, GameEvent::AttackersDeclared { .. })) =>
            {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty attackers are declared");
            }
            Step::DeclareBlockers
                if !game
                    .event_log
                    .iter()
                    .any(|event| matches!(event, GameEvent::BlockersDeclared { .. })) =>
            {
                game.declare_blockers(PlayerId(1 - game.active_player.0), &[])
                    .expect("empty blockers are declared");
            }
            _ => {
                let priority = game.priority;
                game.pass_priority(priority)
                    .expect("priority advances turn state");
            }
        }
    }
    panic!("Flickerform delayed return did not reach the next end step");
}

#[test]
fn flickerform_requires_aura_attachment_and_linked_blink_binding() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FLICKERFORM")
        .expect("Flickerform definition exists");
    assert_eq!(definition.name, "Flickerform");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"aura-linked-exile-and-next-end-step-return")
    );
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-FLICKERFORM"
            && binding.ability.id == "linked-exile-attached-creature-and-auras"
            && binding.ability.mana_cost == ManaCost::with_colors(2, [Color::White, Color::White])
    }));
}

#[test]
#[allow(clippy::too_many_lines)] // The complete blink lifecycle is intentionally one causal trace.
fn flickerform_exiles_the_attached_creature_and_returns_it_with_all_auras() {
    let mut game = rav_game();
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("creature is available to enchant");
    let flickerform = game
        .add_card(PlayerId(0), "RAV-FLICKERFORM", Zone::Hand)
        .expect("Flickerform is in hand");
    let cloak = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Hand)
        .expect("a second Aura is in hand");
    let plains = (0..8)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("Plains enters during setup")
        })
        .collect::<Vec<_>>();
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest enters during setup");
    game.begin_game().expect("game starts");
    advance_to_main(&mut game);
    for land in plains.iter().take(2) {
        game.activate_mana_ability(PlayerId(0), *land, Color::White)
            .expect("Flickerform payment mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: flickerform,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Flickerform casts");
    game.pass_priority(PlayerId(0)).expect("caster passes Aura");
    game.pass_priority(PlayerId(1)).expect("Aura resolves");
    assert_eq!(
        game.object(flickerform).expect("Aura lives").attached_to,
        Some(creature)
    );
    for land in plains.iter().skip(2).take(2) {
        game.activate_mana_ability(PlayerId(0), *land, Color::White)
            .expect("Moldervine Cloak payment mana");
    }
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest pays Moldervine Cloak's green requirement");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: cloak,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("second Aura casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes Cloak");
    game.pass_priority(PlayerId(1)).expect("Cloak resolves");
    for land in plains.iter().skip(4).take(4) {
        game.activate_mana_ability(PlayerId(0), *land, Color::White)
            .expect("Flickerform activation mana");
    }
    game.clear_event_log();
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: flickerform,
            ability_id: "linked-exile-attached-creature-and-auras",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Flickerform activation is paid and stacked");
    game.pass_priority(PlayerId(0))
        .expect("controller passes blink ability");
    game.pass_priority(PlayerId(1))
        .expect("blink ability resolves");
    assert_eq!(game.zone_of(creature), Some(Zone::Exile));
    assert_eq!(game.zone_of(flickerform), Some(Zone::Exile));
    assert_eq!(game.zone_of(cloak), Some(Zone::Exile));
    advance_until_delayed_return(&mut game);
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(flickerform), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(cloak), Some(Zone::Battlefield));
    assert_eq!(
        game.object(flickerform)
            .expect("Flickerform returned")
            .attached_to,
        Some(creature)
    );
    assert_eq!(
        game.object(cloak).expect("Cloak returned").attached_to,
        Some(creature)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DelayedActionScheduled { members, .. }
            if members.len() == 3
                && members.iter().any(|member| member.object == creature)
                && members.iter().any(|member| member.object == flickerform)
                && members.iter().any(|member| member.object == cloak)
    )));
    eprintln!("Flickerform trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Flickerform linked blink preserves invariants");
}
