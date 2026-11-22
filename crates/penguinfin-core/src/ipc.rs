//! What the PenguinFin 2 backend (Tauri) and its interface (Leptos, WebAssembly) send each other.
//! Both sides compile these same types, so a renamed field fails to build instead of at run time.

use crate::fred::{FredConfig, InflationSource, MarketData};
use crate::model::{AppData, Settings};
use serde::{Deserialize, Serialize};

/// Everything the interface shows. Sent after every change, replacing the previous one.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Snapshot {
    /// All of the person's data. `data.fred.api_key` is always empty: the key never enters the webview.
    pub data: AppData,
    pub fred: FredConfig,
    pub market: MarketData,
    pub fred_key: KeyStatus,
    pub db: DbInfo,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct KeyStatus {
    pub set: bool,
    /// Last four characters, to recognise the key.
    pub tail: String,
    /// "the system keychain" or "a private file" (when no keychain is available).
    pub stored_in: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DbInfo {
    pub path: String,
    pub schema_version: i64,
    /// Set when this file was upgraded from PenguinFin 1 (its old data format).
    pub migrated_from: Option<i64>,
}

/// How the app started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BootStatus {
    /// The database opened. `upgraded_backup` = where the copy of a PenguinFin 1 file was saved
    /// when it was upgraded during this start.
    Ready { upgraded_backup: Option<String> },
    /// No database yet: show the welcome screen.
    Welcome,
    /// The database could not be opened; nothing was changed.
    Failed { error: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Boot {
    pub status: BootStatus,
    pub snapshot: Option<Snapshot>,
    /// Where the database is (or will be).
    pub db_path: String,
    /// Today in the computer's time zone, "YYYY-MM-DD" (the interface has no clock of its own).
    pub today: String,
    /// `linux`, `windows`, `macos`, `android` or `ios`.
    pub platform: String,
    /// `desktop` or `phone`.
    pub form: String,
    pub app_version: String,
}

/// A file read for import, waiting for the person to confirm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreview {
    pub file_name: String,
    /// "a PenguinFin 2 export from 1.10.0", "a PenguinFin backup", "a PenguinFin 1 database".
    pub kind: String,
    /// "5 accounts, 15 income, bills and expenses, 3 debts".
    pub summary: String,
    pub net_worth: f64,
    pub total_debt: f64,
    pub currency: String,
}

/// Arguments of commands that take more than one value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FredOptions {
    pub sync_on_launch: bool,
    pub inflation_source: InflationSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsArgs {
    pub settings: Settings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncOutcome {
    pub snapshot: Snapshot,
    /// A sentence for the toast.
    pub message: String,
    pub error: bool,
}
