use rusqlite::Connection;

use crate::card_meta::load_card_meta_map;
use tcg_core::{CardDefId, EffectAst};

#[test]
fn cg_95_hydro_shot_attack_effect_is_patched_from_noop() {
    let path = format!("{}/../data/cards.sqlite", env!("CARGO_MANIFEST_DIR"));
    let conn = Connection::open(path).expect("open cards.sqlite");
    let meta = load_card_meta_map(&conn).expect("load card meta map");

    let kyogre = meta
        .get(&CardDefId::new("CG-95"))
        .expect("CG-95 in card meta");
    let hydro_shot = kyogre
        .attacks
        .iter()
        .find(|a| a.name == "Hydro Shot")
        .expect("Hydro Shot attack");

    match hydro_shot.effect_ast.as_ref() {
        Some(EffectAst::Custom { id, name, .. }) => {
            assert_eq!(id, "CG-95:Hydro Shot");
            assert_eq!(name.as_deref(), Some("Hydro Shot"));
        }
        other => panic!("expected Custom effect for Hydro Shot, got: {other:?}"),
    }
}
