//! Red contract: public full-fidelity choice cards cannot remain ledger-open.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

const LEDGER: &str = include_str!("../../../ENGINE_BUG_LEDGER.md");

fn ledger_row(id: &str) -> &str {
    LEDGER
        .lines()
        .find(|line| line.starts_with(&format!("| `{id}` |")))
        .unwrap_or_else(|| panic!("ledger row {id} is present"))
}

#[test]
fn public_full_fidelity_choice_cards_do_not_remain_open_in_the_ledger() {
    for (definition_id, ledger_id, choice_rule) in [
        (
            "RAV-BLOOD-FUNNEL",
            "blood-funnel-sacrifice-choice-policy-gap",
            "controller-selected-sacrifice-or-counter",
        ),
        (
            "RAV-VINDICTIVE-MOB",
            "vindictive-mob-sacrifice-choice-bound",
            "enter-battlefield-sacrifice-creature",
        ),
    ] {
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition_id),
            "{definition_id} is a public full-fidelity definition"
        );
        let definition = card_definitions()
            .into_iter()
            .find(|definition| definition.id == definition_id)
            .unwrap_or_else(|| panic!("{definition_id} is catalogued"));
        assert!(
            definition.supported_rules.contains(&choice_rule),
            "{definition_id} advertises its policy-choice rule"
        );
        assert!(
            !ledger_row(ledger_id).contains("| Open;"),
            "{ledger_id} contradicts the public full-fidelity choice contract"
        );
    }
}
