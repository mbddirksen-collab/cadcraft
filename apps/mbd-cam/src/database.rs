use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

pub fn default_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("MBD CAM AI")
        .join("manufacturing.sqlite")
}

pub fn initialize(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let connection = Connection::open(path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS controllers(
                id TEXT PRIMARY KEY, name TEXT NOT NULL, provenance TEXT NOT NULL, verified INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS machines(
                id TEXT PRIMARY KEY, name TEXT NOT NULL, controller_id TEXT,
                limits_json TEXT, inventory_status TEXT NOT NULL DEFAULT 'unknown', provenance TEXT NOT NULL,
                FOREIGN KEY(controller_id) REFERENCES controllers(id)
             );
             CREATE TABLE IF NOT EXISTS tools(id TEXT PRIMARY KEY, name TEXT NOT NULL, data_json TEXT, provenance TEXT NOT NULL, verified INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS holders(id TEXT PRIMARY KEY, name TEXT NOT NULL, data_json TEXT, provenance TEXT NOT NULL, verified INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS materials(id TEXT PRIMARY KEY, name TEXT NOT NULL, data_json TEXT, provenance TEXT NOT NULL, verified INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS cutting_data(id TEXT PRIMARY KEY, tool_id TEXT, material_id TEXT, data_json TEXT, provenance TEXT NOT NULL, verified INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS strategies(id TEXT PRIMARY KEY, name TEXT NOT NULL, data_json TEXT, provenance TEXT NOT NULL, verified INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS fixtures(id TEXT PRIMARY KEY, name TEXT NOT NULL, geometry_path TEXT, data_json TEXT, provenance TEXT NOT NULL, verified INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS setup_templates(id TEXT PRIMARY KEY, name TEXT NOT NULL, data_json TEXT, provenance TEXT NOT NULL, verified INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS technology_templates(id TEXT PRIMARY KEY, name TEXT NOT NULL, data_json TEXT, provenance TEXT NOT NULL, approved INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS historical_examples(id TEXT PRIMARY KEY, source_path TEXT, facts_json TEXT, inferred_json TEXT, provenance TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS backup_imports(id TEXT PRIMARY KEY, source_path TEXT NOT NULL, imported_at TEXT, facts_json TEXT, inferred_json TEXT, provenance TEXT NOT NULL);",
        )
        .map_err(|error| error.to_string())?;

    let controllers = [
        ("fanuc", "FANUC"),
        ("heidenhain", "Heidenhain"),
        ("sinumerik-840c", "Sinumerik 840C"),
        ("sinumerik-828d-shopmill", "Sinumerik 828D / ShopMill"),
        ("sinumerik-840d-shopturn", "Sinumerik 840D / ShopTurn"),
        ("sinumerik-840ct", "Sinumerik 840C(T)"),
    ];
    for (id, name) in controllers {
        connection
            .execute(
                "INSERT OR IGNORE INTO controllers(id,name,provenance,verified) VALUES (?1,?2,?3,0)",
                params![id, name, "vom Auftraggeber bereitgestellte Zuordnung; technische Details ungeprüft"],
            )
            .map_err(|error| error.to_string())?;
    }
    let machines = [
        ("sh40", "SH40", Some("fanuc")),
        ("sh400", "SH400", Some("fanuc")),
        ("nh5000", "NH5000", Some("fanuc")),
        ("axa-5000", "AXA 5000", Some("heidenhain")),
        ("axa-5000-1", "AXA 5000-1", Some("heidenhain")),
        ("axa-chantal", "AXA Chantal", Some("sinumerik-840c")),
        ("axa-magda", "AXA Magda", Some("sinumerik-840c")),
        ("dmf-500l", "DMF 500L", Some("heidenhain")),
        ("dmg-m1", "DMG M1", Some("sinumerik-828d-shopmill")),
        ("spinner-lathe", "Spinner-Drehmaschine", Some("sinumerik-840d-shopturn")),
        ("dmg-sprint", "DMG Sprint", Some("sinumerik-840ct")),
        ("gds-epl2k", "Gds epl2k", None),
    ];
    for (id, name, controller) in machines {
        connection
            .execute(
                "INSERT OR IGNORE INTO machines(id,name,controller_id,limits_json,inventory_status,provenance) VALUES (?1,?2,?3,NULL,'unknown',?4)",
                params![id, name, controller, "vom Auftraggeber bereitgestellt; Maschinengrenzen und Bestände unbekannt"],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_separates_facts_and_inferences() {
        let path = std::env::temp_dir().join(format!("mbd-cam-db-test-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        initialize(&path).expect("database init must work");
        let connection = Connection::open(&path).expect("database must open");
        let count: i64 = connection.query_row("SELECT COUNT(*) FROM machines", [], |row| row.get(0)).expect("machine count");
        assert_eq!(count, 12);
        let columns: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_table_info('backup_imports') WHERE name IN ('facts_json','inferred_json')", [], |row| row.get(0))
            .expect("columns");
        assert_eq!(columns, 2);
        drop(connection);
        let _ = std::fs::remove_file(path);
    }
}
