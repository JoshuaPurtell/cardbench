//! Complete public contract for Selesnya Guildmage's activated abilities.

use cardbench_magic_engine::{Effect, ManaCost, TokenSpec};
use cardbench_magic_rav::{rav_activated_ability_bindings, run_all_scenarios};

#[test]
fn selesnya_guildmage_binds_the_exact_centaur_and_anthem_abilities() {
    let abilities = rav_activated_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == "RAV-SELESNYA-GUILDMAGE")
        .collect::<Vec<_>>();
    assert_eq!(abilities.len(), 2);

    let centaur = abilities
        .iter()
        .find(|binding| binding.ability.id == "create-green-centaur")
        .expect("Centaur ability exists");
    assert_eq!(
        centaur.ability.mana_cost,
        ManaCost::with_colors(3, [cardbench_magic_engine::Color::Green])
    );
    assert_eq!(
        centaur.ability.effects,
        [Effect::CreateToken {
            token: TokenSpec::green_centaur(),
            count: 1,
        }]
    );

    let anthem = abilities
        .iter()
        .find(|binding| binding.ability.id == "anthem-controller-creatures")
        .expect("Anthem ability exists");
    assert_eq!(
        anthem.ability.mana_cost,
        ManaCost::with_colors(3, [cardbench_magic_engine::Color::White])
    );
    assert_eq!(
        anthem.ability.effects,
        [Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
            power: 1,
            toughness: 1,
        }]
    );
}

#[test]
fn selesnya_guildmage_public_scenario_creates_a_centaur_then_applies_anthem() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_selesnya_guildmage_centaur_and_anthem")
        .expect("Selesnya Guildmage public scenario exists");
    println!("Selesnya Guildmage full trace: {:?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:061ae0348caca68d");
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("TokenCreated"))
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("ContinuousEffectCreated"))
    );
}
