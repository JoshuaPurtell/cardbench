//! Red regression for the public Blood Funnel policy-decision fixture.

use cardbench_magic_rav::run_public_scenario;

#[test]
fn blood_funnel_public_fixture_submits_its_required_sacrifice_choice() {
    run_public_scenario("rav_blood_funnel_cast_trigger")
        .expect("the public fixture must resolve Blood Funnel's pending decision before passing");
}
