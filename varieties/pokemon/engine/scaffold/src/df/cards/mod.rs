//! Dragon Frontiers card implementations.
//!
//! Each card has its own module with implementations for:
//! - Poke-Powers and Poke-Bodies
//! - Attack modifiers and effects
//! - Triggers and special effects

pub mod df_001_ampharos;
pub mod df_002_feraligatr;
pub mod df_003_heracross;
pub mod df_004_meganium;
pub mod df_005_milotic;
pub mod df_006_nidoking;
pub mod df_007_nidoqueen;
pub mod df_008_ninetales;
pub mod df_009_pinsir;
pub mod df_010_snorlax;
pub mod df_011_togetic;
pub mod df_012_typhlosion;
pub mod df_013_arbok;
pub mod df_014_cloyster;
pub mod df_015_dewgong;
pub mod df_016_gligar;
pub mod df_017_jynx;
pub mod df_018_ledian;
pub mod df_019_lickitung;
pub mod df_020_mantine;
pub mod df_021_quagsire;
pub mod df_022_seadra;
pub mod df_023_tropius;
pub mod df_024_vibrava;
pub mod df_025_xatu;
pub mod df_026_bayleef;
pub mod df_027_croconaw;
pub mod df_028_dragonair;
pub mod df_029_electabuzz;
pub mod df_030_flaaffy;
pub mod df_031_horsea;
pub mod df_032_kirlia;
pub mod df_033_kirlia_delta;
pub mod df_034_nidorina;
pub mod df_035_nidorino;
pub mod df_036_quilava;
pub mod df_037_seadra;
pub mod df_038_shelgon;
pub mod df_039_smeargle;
pub mod df_040_swellow;
pub mod df_041_togepi;
pub mod df_042_vibrava;
pub mod df_043_bagon;
pub mod df_044_chikorita;
pub mod df_045_cyndaquil;
pub mod df_046_dratini;
pub mod df_047_ekans;
pub mod df_048_elekid;
pub mod df_049_feebas;
pub mod df_050_horsea;
pub mod df_051_larvitar;
pub mod df_052_larvitar;
pub mod df_053_ledyba;
pub mod df_054_mareep;
pub mod df_055_natu;
pub mod df_056_nidoran_f;
pub mod df_057_nidoran_m;
pub mod df_058_pupitar;
pub mod df_059_pupitar;
pub mod df_060_ralts;
pub mod df_061_ralts_delta;
pub mod df_062_seel;
pub mod df_063_shellder;
pub mod df_064_smoochum;
pub mod df_065_swablu;
pub mod df_066_taillow;
pub mod df_067_totodile;
pub mod df_068_trapinch;
pub mod df_069_trapinch;
pub mod df_070_vulpix;
pub mod df_071_wooper;
pub mod df_090_altaria_ex;
pub mod df_091_dragonite_ex;
pub mod df_092_flygon_ex;
pub mod df_093_gardevoir_ex;
pub mod df_094_kingdra_ex;
pub mod df_095_latias_ex;
pub mod df_096_latios_ex;
pub mod df_097_rayquaza_ex;
pub mod df_098_salamence_ex;
pub mod df_099_tyranitar_ex;
pub mod df_100_charizard_star;
pub mod df_101_mew_star;

use tcg_core::runtime_hooks::{def_id_matches, AttackOverrides};
use tcg_core::{Attack, CardDefId, CardInstanceId, GameState, PokemonSlot};

pub fn attack_overrides(
    game: &GameState,
    attack: &Attack,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> AttackOverrides {
    let mut overrides = AttackOverrides::default();
    overrides.merge(df_001_ampharos::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_002_feraligatr::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_003_heracross::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_007_nidoqueen::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_009_pinsir::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_015_dewgong::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_016_gligar::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_021_quagsire::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_023_tropius::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_025_xatu::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_032_kirlia::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_035_nidorino::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_037_seadra::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_039_smeargle::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_049_feebas::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_060_ralts::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_065_swablu::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_094_kingdra_ex::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_098_salamence_ex::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(df_099_tyranitar_ex::attack_overrides(game, attack, attacker_id, defender_id));
    overrides
}

pub fn attack_cost_modifier(game: &GameState, attacker_id: CardInstanceId, attack: &Attack) -> i32 {
    df_097_rayquaza_ex::attack_cost_modifier(game, attacker_id, attack)
}

pub fn execute_power(game: &mut GameState, power_name: &str, source_id: CardInstanceId) -> bool {
    match power_name {
        "Dozing" => df_010_snorlax::execute_dozing(game, source_id),
        "Sharing" => df_005_milotic::execute_sharing(game, source_id),
        "Invitation" => df_007_nidoqueen::execute_invitation(game, source_id),
        "Evolutionary Call" => df_004_meganium::execute_evolutionary_call(game, source_id),
        "Volunteer" => df_008_ninetales::execute_volunteer(game, source_id),
        "Shady Move" => df_012_typhlosion::execute_shady_move(game, source_id),
        "Prowl" => df_018_ledian::execute_prowl(game, source_id),
        "Power Circulation" => df_020_mantine::execute_power_circulation(game, source_id),
        "Dig Up" => df_021_quagsire::execute_dig_up(game, source_id),
        "Tropical Heal" => df_023_tropius::execute_tropical_heal(game, source_id),
        "Sand Damage" => df_092_flygon_ex::execute_sand_damage(game, source_id),
        "Imprison" => df_093_gardevoir_ex::execute_imprison(game, source_id),
        "Type Shift" => df_098_salamence_ex::execute_type_shift(game, source_id),
        "Power of Evolution" => df_029_electabuzz::execute_power_of_evolution(game, source_id),
        "Baby Evolution" => crate::df::helpers::execute_baby_evolution(game, source_id),
        "Extra Boost" => df_090_altaria_ex::execute_extra_boost(game, source_id),
        "Fellow Boost" => df_095_latias_ex::execute_fellow_boost(game, source_id),
        "Dark Horn" => df_006_nidoking::execute_dark_horn(game, source_id),
        "Delta Copy" => df_011_togetic::execute_delta_copy(game, source_id),
        "Alluring Kiss" => df_064_smoochum::execute_alluring_kiss(game, source_id),
        "Dragon Roar" => df_091_dragonite_ex::execute_dragon_roar(game, source_id),
        "Dual Stream" => df_098_salamence_ex::execute_dual_stream(game, source_id),
        "Shock-wave" => df_099_tyranitar_ex::execute_shock_wave(game, source_id),
        "Rotating Claws" => df_100_charizard_star::execute_rotating_claws(game, source_id),
        "Mimicry" => df_101_mew_star::execute_mimicry(game, source_id),
        "Rainbow Wave" => df_101_mew_star::execute_rainbow_wave(game, source_id),
        _ => false,
    }
}

pub fn after_attack(
    game: &mut GameState,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) {
    df_098_salamence_ex::after_attack(game, attacker_id, defender_id);
}

pub fn register_triggers(game: &mut GameState, slot: &PokemonSlot) {
    df_004_meganium::register_triggers(game, slot);
    df_014_cloyster::register_triggers(game, slot);
    df_017_jynx::register_triggers(game, slot);
    df_018_ledian::register_triggers(game, slot);
    df_021_quagsire::register_triggers(game, slot);
    df_023_tropius::register_triggers(game, slot);
    df_024_vibrava::register_triggers(game, slot);
    df_040_swellow::register_triggers(game, slot);
    df_092_flygon_ex::register_triggers(game, slot);
    df_096_latios_ex::register_triggers(game, slot);
}

pub fn power_effect_id(def_id: &CardDefId, power_name: &str) -> Option<String> {
    if def_id_matches(def_id, df_004_meganium::SET, df_004_meganium::NUMBER)
        && power_name == "Evolutionary Call"
    {
        return Some(df_004_meganium::evolutionary_call_effect_id());
    }
    if def_id_matches(def_id, df_010_snorlax::SET, df_010_snorlax::NUMBER) && power_name == "Dozing"
    {
        return Some(df_010_snorlax::dozing_effect_id());
    }
    if def_id_matches(def_id, df_005_milotic::SET, df_005_milotic::NUMBER) && power_name == "Sharing"
    {
        return Some(df_005_milotic::sharing_effect_id());
    }
    if def_id_matches(def_id, df_007_nidoqueen::SET, df_007_nidoqueen::NUMBER)
        && power_name == "Invitation"
    {
        return Some(df_007_nidoqueen::invitation_effect_id());
    }
    if def_id_matches(def_id, df_018_ledian::SET, df_018_ledian::NUMBER) && power_name == "Prowl" {
        return Some("DF-18:Prowl".to_string());
    }
    if def_id_matches(def_id, df_020_mantine::SET, df_020_mantine::NUMBER)
        && power_name == "Power Circulation"
    {
        return Some("DF-20:Power Circulation".to_string());
    }
    if def_id_matches(def_id, df_021_quagsire::SET, df_021_quagsire::NUMBER) && power_name == "Dig Up"
    {
        return Some("DF-21:Dig Up".to_string());
    }
    if def_id_matches(def_id, df_023_tropius::SET, df_023_tropius::NUMBER)
        && power_name == "Tropical Heal"
    {
        return Some("DF-23:Tropical Heal".to_string());
    }
    if def_id_matches(def_id, df_093_gardevoir_ex::SET, df_093_gardevoir_ex::NUMBER)
        && power_name == "Imprison"
    {
        return Some("DF-93:Imprison".to_string());
    }
    if def_id_matches(def_id, df_098_salamence_ex::SET, df_098_salamence_ex::NUMBER)
        && power_name == "Type Shift"
    {
        return Some("DF-98:Type Shift".to_string());
    }
    if def_id_matches(def_id, df_008_ninetales::SET, df_008_ninetales::NUMBER)
        && power_name == "Volunteer"
    {
        return Some(df_008_ninetales::volunteer_effect_id());
    }
    if def_id_matches(def_id, df_029_electabuzz::SET, df_029_electabuzz::NUMBER)
        && power_name == "Power of Evolution"
    {
        return Some("DF-29:Power of Evolution".to_string());
    }
    if def_id_matches(def_id, df_048_elekid::SET, df_048_elekid::NUMBER)
        && power_name == "Baby Evolution"
    {
        return Some("DF-48:Baby Evolution".to_string());
    }
    if def_id_matches(def_id, df_064_smoochum::SET, df_064_smoochum::NUMBER)
        && power_name == "Baby Evolution"
    {
        return Some("DF-64:Baby Evolution".to_string());
    }
    if def_id_matches(def_id, df_090_altaria_ex::SET, df_090_altaria_ex::NUMBER)
        && power_name == "Extra Boost"
    {
        return Some("DF-90:Extra Boost".to_string());
    }
    if def_id_matches(def_id, df_095_latias_ex::SET, df_095_latias_ex::NUMBER)
        && power_name == "Fellow Boost"
    {
        return Some("DF-95:Fellow Boost".to_string());
    }
    if def_id_matches(def_id, df_012_typhlosion::SET, df_012_typhlosion::NUMBER)
        && power_name == "Shady Move"
    {
        return Some(df_012_typhlosion::shady_move_effect_id());
    }
    None
}

pub fn power_is_once_per_turn(def_id: &CardDefId, power_name: &str) -> bool {
    (def_id_matches(def_id, df_004_meganium::SET, df_004_meganium::NUMBER)
        && power_name == "Evolutionary Call")
        || (def_id_matches(def_id, df_020_mantine::SET, df_020_mantine::NUMBER)
            && power_name == "Power Circulation")
        || (def_id_matches(def_id, df_021_quagsire::SET, df_021_quagsire::NUMBER)
            && power_name == "Dig Up")
        || (def_id_matches(def_id, df_005_milotic::SET, df_005_milotic::NUMBER)
            && power_name == "Sharing")
        || (def_id_matches(def_id, df_093_gardevoir_ex::SET, df_093_gardevoir_ex::NUMBER)
            && power_name == "Imprison")
        || (def_id_matches(def_id, df_098_salamence_ex::SET, df_098_salamence_ex::NUMBER)
            && power_name == "Type Shift")
        || (def_id_matches(def_id, df_008_ninetales::SET, df_008_ninetales::NUMBER)
            && power_name == "Volunteer")
        || (def_id_matches(def_id, df_029_electabuzz::SET, df_029_electabuzz::NUMBER)
            && power_name == "Power of Evolution")
        || (def_id_matches(def_id, df_048_elekid::SET, df_048_elekid::NUMBER)
            && power_name == "Baby Evolution")
        || (def_id_matches(def_id, df_064_smoochum::SET, df_064_smoochum::NUMBER)
            && power_name == "Baby Evolution")
        || (def_id_matches(def_id, df_090_altaria_ex::SET, df_090_altaria_ex::NUMBER)
            && power_name == "Extra Boost")
        || (def_id_matches(def_id, df_095_latias_ex::SET, df_095_latias_ex::NUMBER)
            && power_name == "Fellow Boost")
        || (def_id_matches(def_id, df_012_typhlosion::SET, df_012_typhlosion::NUMBER)
            && power_name == "Shady Move")
}

pub fn resolve_custom_prompt(
    game: &mut GameState,
    effect_id: &str,
    source_id: Option<CardInstanceId>,
    target_ids: &[CardInstanceId],
) -> bool {
    if let Some(tail) = effect_id.strip_prefix("DF-11:Delta Copy:Attack:") {
        return df_011_togetic::resolve_delta_copy_attack(game, tail, source_id);
    }
    if let Some(tail) = effect_id.strip_prefix("DF-101:Mimicry:Attack:") {
        return df_101_mew_star::resolve_mimicry_attack(game, tail, source_id);
    }
    if let Some(tail) = effect_id.strip_prefix("DF-91:Dragon Roar:") {
        return df_091_dragonite_ex::resolve_dragon_roar_overflow(game, tail, target_ids);
    }
    if let Some(tail) = effect_id.strip_prefix("DF-100:Rotating Claws:Energy:") {
        return df_100_charizard_star::resolve_rotating_claws_energy(
            game, tail, source_id, target_ids,
        );
    }
    match effect_id {
        "DF-20:Power Circulation" => df_020_mantine::resolve_power_circulation(game, source_id),
        "DF-5:Sharing" => df_005_milotic::resolve_sharing(game, source_id, target_ids),
        "DF-93:Imprison" => df_093_gardevoir_ex::resolve_imprison(game, source_id, target_ids),
        "DF-12:Shady Move" => df_012_typhlosion::resolve_shady_move(game, target_ids),
        "DF-8:Volunteer" => df_008_ninetales::resolve_volunteer(game, source_id, target_ids),
        "DF-90:Extra Boost" => df_090_altaria_ex::resolve_extra_boost(game, source_id, target_ids),
        "DF-95:Fellow Boost" => {
            df_095_latias_ex::resolve_fellow_boost(game, source_id, target_ids)
        }
        "DF-6:Dark Horn" => df_006_nidoking::resolve_dark_horn(game, source_id, target_ids),
        "DF-6:Dark Horn:Bench" => {
            df_006_nidoking::resolve_dark_horn_bench(game, source_id, target_ids)
        }
        "DF-11:Delta Copy:Target" => {
            df_011_togetic::resolve_delta_copy_target(game, source_id, target_ids)
        }
        "DF-64:Alluring Kiss:Pokemon" => {
            df_064_smoochum::resolve_alluring_kiss_pokemon(game, source_id, target_ids)
        }
        "DF-64:Alluring Kiss:Energy" => {
            df_064_smoochum::resolve_alluring_kiss_energy(game, source_id, target_ids)
        }
        "DF-91:Dragon Roar" => df_091_dragonite_ex::resolve_dragon_roar(game, source_id),
        "DF-98:Dual Stream:Bench" => {
            df_098_salamence_ex::resolve_dual_stream_bench(game, source_id, target_ids)
        }
        "DF-99:Shock-wave" => df_099_tyranitar_ex::resolve_shock_wave(game, source_id, target_ids),
        "DF-100:Rotating Claws:Discard" => {
            df_100_charizard_star::resolve_rotating_claws_discard(game, source_id, target_ids)
        }
        "DF-100:Rotating Claws" => {
            df_100_charizard_star::resolve_rotating_claws(game, source_id)
        }
        "DF-101:Mimicry:Target" => {
            df_101_mew_star::resolve_mimicry_target(game, source_id, target_ids)
        }
        "DF-101:Rainbow Wave" => df_101_mew_star::resolve_rainbow_wave(game, source_id, target_ids),
        _ => false,
    }
}
pub use df_001_ampharos as ampharos;
pub use df_002_feraligatr as feraligatr;
pub use df_003_heracross as heracross;
pub use df_004_meganium as meganium;
pub use df_005_milotic as milotic;
pub use df_006_nidoking as nidoking;
pub use df_007_nidoqueen as nidoqueen;
pub use df_008_ninetales as ninetales;
pub use df_009_pinsir as pinsir;
pub use df_010_snorlax as snorlax;
pub use df_011_togetic as togetic;
pub use df_012_typhlosion as typhlosion;
pub use df_013_arbok as arbok;
pub use df_014_cloyster as cloyster;
pub use df_015_dewgong as dewgong;
pub use df_016_gligar as gligar;
pub use df_017_jynx as jynx;
pub use df_018_ledian as ledian;
pub use df_019_lickitung as lickitung;
pub use df_020_mantine as mantine;
pub use df_021_quagsire as quagsire;
pub use df_022_seadra as seadra;
pub use df_023_tropius as tropius;
pub use df_024_vibrava as vibrava_24;
pub use df_025_xatu as xatu;
pub use df_026_bayleef as bayleef;
pub use df_027_croconaw as croconaw;
pub use df_028_dragonair as dragonair;
pub use df_029_electabuzz as electabuzz;
pub use df_030_flaaffy as flaaffy;
pub use df_031_horsea as horsea_31;
pub use df_032_kirlia as kirlia;
pub use df_033_kirlia_delta as kirlia_delta;
pub use df_034_nidorina as nidorina;
pub use df_035_nidorino as nidorino;
pub use df_036_quilava as quilava;
pub use df_037_seadra as seadra_delta;
pub use df_038_shelgon as shelgon;
pub use df_039_smeargle as smeargle;
pub use df_040_swellow as swellow;
pub use df_041_togepi as togepi;
pub use df_042_vibrava as vibrava_42;
pub use df_043_bagon as bagon;
pub use df_044_chikorita as chikorita;
pub use df_045_cyndaquil as cyndaquil;
pub use df_046_dratini as dratini;
pub use df_047_ekans as ekans;
pub use df_048_elekid as elekid;
pub use df_049_feebas as feebas;
pub use df_050_horsea as horsea_50;
pub use df_051_larvitar as larvitar;
pub use df_052_larvitar as larvitar_delta;
pub use df_053_ledyba as ledyba;
pub use df_054_mareep as mareep;
pub use df_055_natu as natu;
pub use df_056_nidoran_f as nidoran_f;
pub use df_057_nidoran_m as nidoran_m;
pub use df_058_pupitar as pupitar;
pub use df_059_pupitar as pupitar_delta;
pub use df_060_ralts as ralts;
pub use df_061_ralts_delta as ralts_delta;
pub use df_062_seel as seel;
pub use df_063_shellder as shellder;
pub use df_064_smoochum as smoochum;
pub use df_065_swablu as swablu;
pub use df_066_taillow as taillow;
pub use df_067_totodile as totodile;
pub use df_068_trapinch as trapinch_68;
pub use df_069_trapinch as trapinch_69;
pub use df_070_vulpix as vulpix;
pub use df_071_wooper as wooper;
pub use df_090_altaria_ex as altaria_ex;
pub use df_091_dragonite_ex as dragonite_ex;
pub use df_092_flygon_ex as flygon_ex;
pub use df_093_gardevoir_ex as gardevoir_ex;
pub use df_094_kingdra_ex as kingdra_ex;
pub use df_095_latias_ex as latias_ex;
pub use df_096_latios_ex as latios_ex;
pub use df_097_rayquaza_ex as rayquaza_ex;
pub use df_098_salamence_ex as salamence_ex;
pub use df_099_tyranitar_ex as tyranitar_ex;
pub use df_100_charizard_star as charizard_star;
pub use df_101_mew_star as mew_star;
