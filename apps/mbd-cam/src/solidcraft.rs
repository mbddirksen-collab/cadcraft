use std::{
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolidCraftReport {
    pub version: String,
    pub body_name: String,
    pub faces: usize,
    pub edges: usize,
    pub exact_faces: usize,
    pub message: String,
}

pub fn inspect(cli: &Path, step: &Path) -> Result<SolidCraftReport, String> {
    if !cli.is_file() {
        return Err(format!("SolidCraft CLI nicht gefunden: {}", cli.display()));
    }
    let version_output = Command::new(cli).arg("--version").output().map_err(|error| error.to_string())?;
    if !version_output.status.success() {
        return Err("SolidCraft --version ist fehlgeschlagen".into());
    }
    let version = String::from_utf8_lossy(&version_output.stdout).trim().to_owned();
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let temp_dir = std::env::temp_dir();
    let measure_path = temp_dir.join(format!("mbd-cam-solidcraft-{}-{nonce}-measure.json", std::process::id()));
    let query_path = temp_dir.join(format!("mbd-cam-solidcraft-{}-{nonce}-query.json", std::process::id()));
    let result_path = temp_dir.join(format!("mbd-cam-solidcraft-{}-{nonce}-result.json", std::process::id()));
    let measure = Command::new(cli).arg("eval").arg(step).arg("--json-out").arg(&measure_path).output().map_err(|error| error.to_string())?;
    if !measure.status.success() {
        return Err(format!("SolidCraft STEP-Prüfung fehlgeschlagen: {}", String::from_utf8_lossy(&measure.stderr).trim()));
    }
    let measurement: serde_json::Value = read_json(&measure_path)?;
    let body_name = measurement
        .pointer("/bodies/0/name")
        .and_then(serde_json::Value::as_str)
        .or_else(|| measurement.pointer("/0/bodies/0/name").and_then(serde_json::Value::as_str))
        .ok_or_else(|| "SolidCraft meldet keinen Körpernamen".to_owned())?
        .to_owned();
    let query = serde_json::json!([
        {"command":"model.faces","params":{"body":body_name}},
        {"command":"model.edges","params":{"body":body_name}}
    ]);
    let query_text = serde_json::to_string_pretty(&query).map_err(|error| error.to_string())?;
    std::fs::write(&query_path, query_text).map_err(|error| error.to_string())?;
    let output = Command::new(cli)
        .arg("run")
        .arg(&query_path)
        .arg("--in")
        .arg(step)
        .arg("--results")
        .arg("--param-check")
        .arg("strict")
        .arg("--json-out")
        .arg(&result_path)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        cleanup(&[&measure_path, &query_path, &result_path]);
        return Err(format!("SolidCraft Flächenabfrage fehlgeschlagen: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    let result: serde_json::Value = read_json(&result_path)?;
    let faces = result.pointer("/0/faces").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
    let edges = result.pointer("/1/edges").and_then(serde_json::Value::as_array).map_or(0, Vec::len);
    let exact_faces = faces.iter().filter(|face| face.get("exact").and_then(serde_json::Value::as_bool) == Some(true)).count();
    cleanup(&[&measure_path, &query_path, &result_path]);
    Ok(SolidCraftReport {
        version,
        body_name,
        faces: faces.len(),
        edges,
        exact_faces,
        message: "STEP importiert; B-Rep-Flächen und -Kanten programmatisch abgefragt.".into(),
    })
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err("SolidCraft-Ergebnis überschreitet 128 MB".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn cleanup(paths: &[&Path]) {
    for path in paths {
        let _ = std::fs::remove_file(path);
    }
}
