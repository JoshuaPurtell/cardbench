use rusqlite::{params, Connection, Result};

struct EnergySpec {
    def_id: &'static str,
    name: &'static str,
    provides: &'static str,
}

const BASIC_ENERGIES: &[EnergySpec] = &[
    EnergySpec {
        def_id: "ENERGY-GRASS",
        name: "Grass Energy",
        provides: "Grass",
    },
    EnergySpec {
        def_id: "ENERGY-FIRE",
        name: "Fire Energy",
        provides: "Fire",
    },
    EnergySpec {
        def_id: "ENERGY-WATER",
        name: "Water Energy",
        provides: "Water",
    },
    EnergySpec {
        def_id: "ENERGY-LIGHTNING",
        name: "Lightning Energy",
        provides: "Lightning",
    },
    EnergySpec {
        def_id: "ENERGY-PSYCHIC",
        name: "Psychic Energy",
        provides: "Psychic",
    },
    EnergySpec {
        def_id: "ENERGY-FIGHTING",
        name: "Fighting Energy",
        provides: "Fighting",
    },
    EnergySpec {
        def_id: "ENERGY-DARKNESS",
        name: "Darkness Energy",
        provides: "Darkness",
    },
    EnergySpec {
        def_id: "ENERGY-METAL",
        name: "Metal Energy",
        provides: "Metal",
    },
    EnergySpec {
        def_id: "ENERGY-COLORLESS",
        name: "Colorless Energy",
        provides: "Colorless",
    },
];

pub fn add_basic_energies(conn: &Connection) -> Result<usize> {
    conn.execute(
        "INSERT OR IGNORE INTO sets (code, name, era) VALUES (?, ?, ?)",
        params!["ENERGY", "Basic Energy", "EX"],
    )?;

    let set_id: i64 = conn.query_row(
        "SELECT set_id FROM sets WHERE code = ?",
        params!["ENERGY"],
        |row| row.get(0),
    )?;

    let mut inserted = 0;
    for spec in BASIC_ENERGIES {
        let number = spec
            .def_id
            .split('-')
            .nth(1)
            .unwrap_or(spec.def_id);
        let payload = serde_json::json!({ "provides": [spec.provides] }).to_string();
        conn.execute(
            "INSERT OR REPLACE INTO cards (
                card_def_id, set_id, number, name, supertype,
                subtypes_json, tags_json, energy_kind,
                script_kind, script_payload
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                spec.def_id,
                set_id,
                number,
                spec.name,
                "Energy",
                "[]",
                "[]",
                "Basic",
                "Dsl",
                payload
            ],
        )?;
        inserted += 1;
    }

    Ok(inserted)
}
