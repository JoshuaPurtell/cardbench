//! Red discovery contract for Blood Funnel's reduction and counter-unless-sacrifice trigger.

use cardbench_magic_engine::{Game, PlayerId, Zone};
use cardbench_magic_rav::{card_definitions, executable_definition_id_for_collector};

#[test]
fn blood_funnel_is_not_catalog_only() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog constructs");
    println!("Blood Funnel red event log before definition lookup: {:?}", game.event_log);
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BLOOD-FUNNEL")
        .expect("Blood Funnel definition exists");
    let _ = game
        .add_card(PlayerId(0), definition.id, Zone::Battlefield)
        .expect("the red fixture would place Blood Funnel on the battlefield");
    assert_eq!(
        executable_definition_id_for_collector(77),
        Ok("RAV-BLOOD-FUNNEL")
    );
}
