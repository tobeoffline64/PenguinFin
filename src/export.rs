//! Moving data between PenguinFin installs and versions.
//!
//! - `.pfexport`: the export file for moving to PenguinFin 2 (or another computer). JSON with a
//!   header saying what made it and a SHA-256 of the data, so a damaged or newer file is caught
//!   before anything is replaced.
//! - Plain JSON backups from any 1.x release (`penguinfin-backup-*.json`).
//! - A `penguinfin.db` copied from another computer, read without changing it.
//!
//! The FRED API key is never written to an export, the same as backups.

use crate::db::{Db, SCHEMA_VERSION};
use crate::model::AppData;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

pub const FORMAT: &str = "penguinfin-export";
pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "pfexport";

type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub accounts: usize,
    pub buckets: usize,
    pub holdings: usize,
    pub flows: usize,
    pub debts: usize,
    pub debt_payments: usize,
    pub goals: usize,
}

impl Counts {
    pub fn of(d: &AppData) -> Self {
        Counts {
            accounts: d.accounts.len(),
            buckets: d.buckets.len(),
            holdings: d.holdings.len(),
            flows: d.flows.len(),
            debts: d.debts.len(),
            debt_payments: d.debt_payments.len(),
            goals: d.goals.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub format: String,
    pub format_version: u32,
    pub app_version: String,
    pub schema_version: i64,
    pub exported_at: String,
    pub counts: Counts,
    pub sha256: String,
}

/// What kind of file an import came from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Export(Header),
    JsonBackup,
    Database { schema_version: i64 },
}

#[derive(Debug, Clone)]
pub struct Imported {
    pub source: Source,
    pub data: AppData,
}

impl Imported {
    pub fn counts(&self) -> Counts {
        Counts::of(&self.data)
    }
}

/// The data part, serialized the same way every time: compact, keys sorted.
fn canonical(data: &Value) -> String {
    data.to_string()
}

fn digest(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// The `.pfexport` text for `data`. `app_version` names the release writing it.
pub fn to_export_string(data: &AppData, app_version: &str, exported_at: &str) -> Result<String> {
    let mut data = data.clone();
    data.fred.api_key.clear();
    let data_value = serde_json::to_value(&data).map_err(|e| e.to_string())?;
    let header = Header {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        app_version: app_version.into(),
        schema_version: SCHEMA_VERSION,
        exported_at: exported_at.into(),
        counts: Counts::of(&data),
        sha256: digest(&canonical(&data_value)),
    };
    let mut file = serde_json::to_value(&header).map_err(|e| e.to_string())?;
    file.as_object_mut().expect("header is an object").insert("data".into(), data_value);
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

pub fn write_export(data: &AppData, path: &Path, app_version: &str) -> Result<()> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let text = to_export_string(data, app_version, &now)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Read a `.pfexport` or a 1.x JSON backup from text.
pub fn from_json_str(text: &str) -> Result<Imported> {
    let mut v: Value = serde_json::from_str(text).map_err(|e| format!("not a PenguinFin file ({e})"))?;
    let is_export = v.get("format").and_then(Value::as_str) == Some(FORMAT);
    if !is_export {
        let data: AppData = serde_json::from_value(v).map_err(|e| format!("not a PenguinFin backup ({e})"))?;
        return Ok(Imported { source: Source::JsonBackup, data });
    }
    let data_value = v.as_object_mut().and_then(|o| o.remove("data")).ok_or("the export has no data")?;
    let header: Header = serde_json::from_value(v).map_err(|e| format!("the export's header is damaged ({e})"))?;
    if header.format_version > FORMAT_VERSION {
        return Err(format!(
            "this export was made by a newer PenguinFin ({}); update PenguinFin to open it",
            header.app_version
        ));
    }
    if digest(&canonical(&data_value)) != header.sha256 {
        return Err("the export is damaged: its contents don't match its checksum".into());
    }
    let data: AppData = serde_json::from_value(data_value).map_err(|e| format!("the export's data is damaged ({e})"))?;
    if Counts::of(&data) != header.counts {
        return Err("the export is damaged: it holds a different number of items than its header says".into());
    }
    Ok(Imported { source: Source::Export(header), data })
}

/// Read any file PenguinFin can import: `.pfexport`, a JSON backup, or a `penguinfin.db`.
pub fn read_file(path: &Path) -> Result<Imported> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.starts_with(b"SQLite format 3\0") {
        let db = Db::open_copy(path)?;
        let schema_version = db.schema_version()?;
        let mut data = db.load()?;
        data.fred.api_key.clear();
        return Ok(Imported { source: Source::Database { schema_version }, data });
    }
    let text = String::from_utf8(bytes).map_err(|_| "not a PenguinFin file".to_string())?;
    from_json_str(&text)
}

/// One line for a confirmation: "5 accounts, 21 income and bills, 3 debts, 2 payments".
pub fn summary(c: &Counts) -> String {
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut parts = vec![
        plural(c.accounts, "account", "accounts"),
        plural(c.flows, "income, bill or expense", "income, bills and expenses"),
    ];
    if c.buckets > 0 {
        parts.push(plural(c.buckets, "bucket", "buckets"));
    }
    if c.debts > 0 {
        parts.push(plural(c.debts, "debt", "debts"));
    }
    if c.debt_payments > 0 {
        parts.push(plural(c.debt_payments, "debt payment", "debt payments"));
    }
    if c.goals > 0 {
        parts.push(plural(c.goals, "goal", "goals"));
    }
    if c.holdings > 0 {
        parts.push(plural(c.holdings, "holding", "holdings"));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    /// Everything a person would notice if an upgrade went wrong.
    fn fingerprint(d: &AppData) -> (Counts, i64, i64, i64) {
        let cents = |v: f64| (v * 100.0).round() as i64;
        let accounts: f64 = d.accounts.iter().map(|a| a.balance).sum();
        let flows: f64 = d.flows.iter().map(|f| f.amount).sum();
        (Counts::of(d), cents(accounts), cents(d.total_debt()), cents(flows))
    }

    /// Load into a fresh database the way a restore does, then read it back.
    fn restored(data: &AppData) -> AppData {
        let mut db = Db::open_in_memory().unwrap();
        db.replace_all(data).unwrap();
        db.load().unwrap()
    }

    #[test]
    fn every_old_schema_upgrades_and_round_trips() {
        for v in 1..=SCHEMA_VERSION {
            let db_path = fixture(&format!("schema-v{v}.db"));
            // The file is read, never changed.
            let before = std::fs::read(&db_path).unwrap();
            let imported = read_file(&db_path).unwrap_or_else(|e| panic!("schema v{v}: {e}"));
            assert_eq!(imported.source, Source::Database { schema_version: SCHEMA_VERSION }, "v{v}");
            assert_eq!(std::fs::read(&db_path).unwrap(), before, "v{v} file was modified");
            let upgraded = imported.data;
            assert!(upgraded.accounts.len() >= 5 && !upgraded.flows.is_empty(), "v{v}");

            // The same release's own JSON backup holds the same money.
            let backup = read_file(&fixture(&format!("schema-v{v}.json"))).unwrap();
            assert_eq!(backup.source, Source::JsonBackup);
            assert_eq!(fingerprint(&restored(&backup.data)), fingerprint(&upgraded), "v{v} backup vs database");

            // Export → import → restore gives back exactly what was exported.
            let text = to_export_string(&upgraded, "test", "2026-11-21T00:00:00Z").unwrap();
            let back = from_json_str(&text).unwrap();
            assert!(matches!(back.source, Source::Export(ref h) if h.schema_version == SCHEMA_VERSION));
            assert_eq!(fingerprint(&restored(&back.data)), fingerprint(&upgraded), "v{v} export round trip");
        }
    }

    #[test]
    fn payment_history_survives_without_being_applied_twice() {
        let d = read_file(&fixture("schema-v7.db")).unwrap().data;
        assert_eq!(d.debt_payments.len(), 2);
        let text = to_export_string(&d, "test", "2026-11-21T00:00:00Z").unwrap();
        let back = restored(&from_json_str(&text).unwrap().data);
        assert_eq!(back.debt_payments.len(), 2);
        assert!(back.debt_payments.iter().any(|p| p.logged && p.applied == 500.0));
        assert!(back.debt_payments.iter().any(|p| !p.logged && p.amount == 300.0));
        assert_eq!(back.total_debt(), d.total_debt());
    }

    #[test]
    fn damaged_or_newer_exports_are_refused() {
        let d = crate::sample::sample_data(chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        let text = to_export_string(&d, "test", "2026-11-21T00:00:00Z").unwrap();
        let mut v: Value = serde_json::from_str(&text).unwrap();
        v["data"]["accounts"][0]["balance"] = serde_json::json!(1.5);
        let tampered = serde_json::to_string_pretty(&v).unwrap();
        assert!(from_json_str(&tampered).unwrap_err().contains("damaged"));
        let newer = text.replacen("\"format_version\": 1", "\"format_version\": 2", 1);
        assert!(from_json_str(&newer).unwrap_err().contains("newer"));
        assert!(from_json_str("not json").is_err());
    }

    #[test]
    fn exports_never_contain_the_api_key() {
        let mut d = crate::sample::sample_data(chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        d.fred.api_key = "secretsecretsecretsecretsecret12".into();
        let text = to_export_string(&d, "test", "2026-11-21T00:00:00Z").unwrap();
        assert!(!text.contains("secretsecret"));
    }

    #[test]
    fn newer_databases_are_refused_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("penguinfin.db");
        std::fs::copy(fixture("schema-v7.db"), &path).unwrap();
        rusqlite::Connection::open(&path).unwrap().execute_batch("PRAGMA user_version = 8;").unwrap();
        let err = Db::open(&path).err().expect("must refuse");
        assert!(err.contains("newer PenguinFin"), "{err}");
        let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 8);
    }
}
