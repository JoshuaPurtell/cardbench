//! Red regression for a public scenario grammar gap in choice mana abilities.

use cardbench_magic_rav::run_all_scenarios;

#[test]
fn public_cast_payment_fixture_must_retain_the_selected_choice_color() {
    let scenarios = run_all_scenarios()
        .expect("a shown fixture may select one legal color for a bound cast-payment mana ability");
    assert!(
        scenarios
            .iter()
            .any(|scenario| scenario.id == "rav_birds_of_paradise_flying_cast_payment")
    );
}
