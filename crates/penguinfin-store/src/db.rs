//! Local SQLite storage. Everything lives in a single file in the user's data dir.

use crate::debt::{Debt, DebtKind, DebtPayment, Strategy};
use crate::fred::{FredConfig, Indicator, InflationSource, MarketData, MarketValue};
use crate::model::*;
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

/// `$XDG_DATA_HOME/penguinfin` (inside the flatpak: `~/.var/app/<id>/data/penguinfin`).
///
/// Windows: `%APPDATA%\PenguinFin`. macOS: `~/Library/Application Support/PenguinFin`.
pub fn data_dir() -> PathBuf {
    let abs = |v: &str| std::env::var_os(v).map(PathBuf::from).filter(|p| p.is_absolute());
    if cfg!(windows) {
        let base = abs("APPDATA").or_else(|| abs("USERPROFILE").map(|h| h.join("AppData").join("Roaming")));
        return base.unwrap_or_else(|| PathBuf::from(".")).join("PenguinFin");
    }
    if cfg!(target_os = "macos")
        && let Some(home) = abs("HOME")
    {
        return home.join("Library/Application Support/PenguinFin");
    }
    let base = abs("XDG_DATA_HOME")
        .or_else(|| abs("HOME").map(|h| h.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("penguinfin")
}

/// The user's home folder (`%USERPROFILE%` on Windows).
pub fn home_dir() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

pub fn default_db_path() -> PathBuf {
    data_dir().join("penguinfin.db")
}

/// The database format this version writes (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i64 = 8;

pub struct Db {
    conn: Connection,
    path: PathBuf,
}

fn date_to_sql(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn date_from_sql(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap_or_else(|_| chrono::Local::now().date_naive())
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(e)?;
        }
        let conn = Connection::open(path).map_err(e)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;").map_err(e)?;
        let db = Db { conn, path: path.to_path_buf() };
        db.migrate()?;
        Ok(db)
    }

    /// Open a copy of another database file in memory, upgraded to this version's format.
    /// Nothing is written next to the original (e.g. a `penguinfin.db` from another computer):
    /// the file and its write-ahead log, if any, are copied to a private temporary folder first.
    pub fn open_copy(path: &Path) -> Result<Self> {
        use rusqlite::OpenFlags;
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("penguinfin-import-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(e)?;
        let result = (|| {
            let tmp = dir.join("import.db");
            std::fs::copy(path, &tmp).map_err(e)?;
            let mut wal = path.as_os_str().to_owned();
            wal.push("-wal");
            if Path::new(&wal).exists() {
                std::fs::copy(&wal, dir.join("import.db-wal")).map_err(e)?;
            }
            let src = Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
                .map_err(e)?;
            let mut conn = Connection::open_in_memory().map_err(e)?;
            {
                let backup = rusqlite::backup::Backup::new(&src, &mut conn).map_err(e)?;
                backup.run_to_completion(256, std::time::Duration::ZERO, None).map_err(e)?;
            }
            conn.execute_batch("PRAGMA foreign_keys = ON;").map_err(e)?;
            let db = Db { conn, path: path.to_path_buf() };
            db.migrate()?;
            Ok(db)
        })();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    /// Open the app's own database, upgrading an older one. Before any upgrade a copy of the
    /// file is saved beside it as `penguinfin-<from>-backup-YYYYMMDD-HHMMSS.db`; if the upgrade
    /// fails, the original is put back and the error returned. Returns the copy's path.
    pub fn open_upgrading(path: &Path) -> Result<(Self, Option<PathBuf>)> {
        let old_version = if path.exists() {
            let c = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(e)?;
            c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).map_err(e)?
        } else {
            0
        };
        if old_version == 0 || old_version >= SCHEMA_VERSION {
            return Ok((Db::open(path)?, None));
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let label = if old_version < 8 { "1.x".to_string() } else { format!("v{old_version}") };
        let backup = path.with_file_name(format!("penguinfin-{label}-backup-{stamp}.db"));
        {
            // The backup API copies a consistent snapshot, including anything still in the WAL.
            let src = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(e)?;
            let mut dst = Connection::open(&backup).map_err(e)?;
            let b = rusqlite::backup::Backup::new(&src, &mut dst).map_err(e)?;
            b.run_to_completion(256, std::time::Duration::ZERO, None).map_err(e)?;
        }
        match Db::open(path) {
            Ok(db) => Ok((db, Some(backup))),
            Err(err) => {
                // Put the original back: copy the snapshot over it and drop a stale WAL.
                for side in ["-wal", "-shm"] {
                    let mut p = path.as_os_str().to_owned();
                    p.push(side);
                    let _ = std::fs::remove_file(PathBuf::from(p));
                }
                std::fs::copy(&backup, path).map_err(e)?;
                Err(format!("{err}. Your data was left as it was; a copy is at {}", backup.display()))
            }
        }
    }

    /// Copy the whole database to `dest` (a consistent snapshot, safe while in use).
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        let mut out = Connection::open(dest).map_err(e)?;
        let b = rusqlite::backup::Backup::new(&self.conn, &mut out).map_err(e)?;
        b.run_to_completion(256, std::time::Duration::ZERO, None).map_err(e)
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        self.conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional().map_err(e)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(e)?;
        Ok(())
    }

    /// Remove the FRED key from the database (2.0 keeps it in the OS keychain instead).
    pub fn clear_fred_key(&self) -> Result<()> {
        self.conn.execute("DELETE FROM settings WHERE key = 'fred_api_key'", []).map_err(e)?;
        Ok(())
    }

    /// If the inflation assumption follows a FRED indicator, copy its latest value (rounded to
    /// 0.1%) into the inflation setting used by projections.
    pub fn apply_market_inflation(&mut self) -> Result<()> {
        let data = self.load()?;
        let Some(indicator) = data.fred.inflation_source.indicator() else { return Ok(()) };
        let Some(v) = data.market.get(indicator) else { return Ok(()) };
        let rounded = (v.value * 10.0).round() / 10.0;
        if (data.settings.inflation_pct - rounded).abs() > f64::EPSILON {
            let mut s = data.settings.clone();
            s.inflation_pct = rounded;
            self.save_settings(&s)?;
        }
        Ok(())
    }

    /// The data format of the open database.
    pub fn schema_version(&self) -> Result<i64> {
        self.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(e)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(e)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;").map_err(e)?;
        let db = Db { conn, path: PathBuf::from(":memory:") };
        db.migrate()?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn migrate(&self) -> Result<()> {
        let version: i64 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(e)?;
        if version > SCHEMA_VERSION {
            return Err(format!(
                "this data file was saved by a newer PenguinFin (data format {version}; this version reads up to {SCHEMA_VERSION}). Open it with PenguinFin 2, or restore a backup"
            ));
        }
        if version < 1 {
            self.conn
                .execute_batch(
                    "BEGIN;
                CREATE TABLE IF NOT EXISTS cash_flows (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    kind TEXT NOT NULL,
                    name TEXT NOT NULL,
                    category TEXT NOT NULL DEFAULT '',
                    amount REAL NOT NULL,
                    frequency TEXT NOT NULL,
                    start_date TEXT NOT NULL,
                    end_date TEXT,
                    annual_change_pct REAL NOT NULL DEFAULT 0,
                    autopay INTEGER NOT NULL DEFAULT 0,
                    notes TEXT NOT NULL DEFAULT ''
                );
                CREATE TABLE IF NOT EXISTS accounts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    balance REAL NOT NULL DEFAULT 0,
                    annual_return_pct REAL NOT NULL DEFAULT 0,
                    monthly_contribution REAL NOT NULL DEFAULT 0,
                    contribution_from_cash INTEGER NOT NULL DEFAULT 0,
                    employer_match_monthly REAL NOT NULL DEFAULT 0,
                    notes TEXT NOT NULL DEFAULT ''
                );
                CREATE TABLE IF NOT EXISTS holdings (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                    symbol TEXT NOT NULL,
                    shares REAL NOT NULL,
                    price REAL NOT NULL,
                    price_updated TEXT
                );
                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                PRAGMA user_version = 1;
                COMMIT;",
                )
                .map_err(e)?;
        }
        if version < 2 {
            self.conn
                .execute_batch(
                    "BEGIN;
                CREATE TABLE IF NOT EXISTS market_data (
                    indicator TEXT PRIMARY KEY,
                    value REAL NOT NULL,
                    as_of TEXT NOT NULL
                );
                PRAGMA user_version = 2;
                COMMIT;",
                )
                .map_err(e)?;
        }
        if version < 3 {
            self.conn
                .execute_batch(
                    "BEGIN;
                ALTER TABLE accounts ADD COLUMN purpose TEXT NOT NULL DEFAULT '';
                ALTER TABLE accounts ADD COLUMN minimum_balance REAL NOT NULL DEFAULT 0;
                CREATE TABLE IF NOT EXISTS goals (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    cost REAL NOT NULL,
                    target_date TEXT,
                    priority INTEGER NOT NULL DEFAULT 0,
                    notes TEXT NOT NULL DEFAULT '',
                    purchased_on TEXT
                );
                PRAGMA user_version = 3;
                COMMIT;",
                )
                .map_err(e)?;
        }
        if version < 4 {
            self.conn
                .execute_batch(
                    "BEGIN;
                CREATE TABLE IF NOT EXISTS buckets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    balance REAL NOT NULL DEFAULT 0,
                    monthly_add REAL NOT NULL DEFAULT 0,
                    target REAL,
                    notes TEXT NOT NULL DEFAULT ''
                );
                ALTER TABLE cash_flows ADD COLUMN account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL;
                ALTER TABLE cash_flows ADD COLUMN bucket_id INTEGER REFERENCES buckets(id) ON DELETE SET NULL;
                PRAGMA user_version = 4;
                COMMIT;",
                )
                .map_err(e)?;
        }
        if version < 5 {
            self.conn
                .execute_batch(
                    "BEGIN;
                CREATE TABLE IF NOT EXISTS debts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    balance REAL NOT NULL,
                    apr REAL NOT NULL DEFAULT 0,
                    min_payment REAL NOT NULL DEFAULT 0,
                    account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
                    notes TEXT NOT NULL DEFAULT ''
                );
                PRAGMA user_version = 5;
                COMMIT;",
                )
                .map_err(e)?;
        }
        if version < 6 {
            self.conn
                .execute_batch(
                    "BEGIN;
                ALTER TABLE buckets ADD COLUMN debt_id INTEGER REFERENCES debts(id) ON DELETE CASCADE;
                PRAGMA user_version = 6;
                COMMIT;",
                )
                .map_err(e)?;
        }
        if version < 7 {
            self.conn
                .execute_batch(
                    "BEGIN;
                CREATE TABLE IF NOT EXISTS debt_payments (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    debt_id INTEGER NOT NULL REFERENCES debts(id) ON DELETE CASCADE,
                    date TEXT NOT NULL,
                    amount REAL NOT NULL,
                    account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
                    notes TEXT NOT NULL DEFAULT '',
                    logged INTEGER NOT NULL DEFAULT 0,
                    applied REAL NOT NULL DEFAULT 0,
                    deducted INTEGER NOT NULL DEFAULT 0
                );
                PRAGMA user_version = 7;
                COMMIT;",
                )
                .map_err(e)?;
        }
        if version < 8 {
            // 2.0: a meta table (where the file came from) and updated_at on every row,
            // kept current by triggers so every write path stamps it.
            self.conn
                .execute_batch(
                    "BEGIN;
                CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                INSERT OR IGNORE INTO meta (key, value) VALUES ('created_at', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));
                ALTER TABLE cash_flows ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
                UPDATE cash_flows SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');
                CREATE TRIGGER IF NOT EXISTS cash_flows_inserted AFTER INSERT ON cash_flows BEGIN UPDATE cash_flows SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                CREATE TRIGGER IF NOT EXISTS cash_flows_updated AFTER UPDATE ON cash_flows WHEN NEW.updated_at = OLD.updated_at BEGIN UPDATE cash_flows SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                ALTER TABLE accounts ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
                UPDATE accounts SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');
                CREATE TRIGGER IF NOT EXISTS accounts_inserted AFTER INSERT ON accounts BEGIN UPDATE accounts SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                CREATE TRIGGER IF NOT EXISTS accounts_updated AFTER UPDATE ON accounts WHEN NEW.updated_at = OLD.updated_at BEGIN UPDATE accounts SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                ALTER TABLE holdings ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
                UPDATE holdings SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');
                CREATE TRIGGER IF NOT EXISTS holdings_inserted AFTER INSERT ON holdings BEGIN UPDATE holdings SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                CREATE TRIGGER IF NOT EXISTS holdings_updated AFTER UPDATE ON holdings WHEN NEW.updated_at = OLD.updated_at BEGIN UPDATE holdings SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                ALTER TABLE goals ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
                UPDATE goals SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');
                CREATE TRIGGER IF NOT EXISTS goals_inserted AFTER INSERT ON goals BEGIN UPDATE goals SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                CREATE TRIGGER IF NOT EXISTS goals_updated AFTER UPDATE ON goals WHEN NEW.updated_at = OLD.updated_at BEGIN UPDATE goals SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                ALTER TABLE buckets ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
                UPDATE buckets SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');
                CREATE TRIGGER IF NOT EXISTS buckets_inserted AFTER INSERT ON buckets BEGIN UPDATE buckets SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                CREATE TRIGGER IF NOT EXISTS buckets_updated AFTER UPDATE ON buckets WHEN NEW.updated_at = OLD.updated_at BEGIN UPDATE buckets SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                ALTER TABLE debts ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
                UPDATE debts SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');
                CREATE TRIGGER IF NOT EXISTS debts_inserted AFTER INSERT ON debts BEGIN UPDATE debts SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                CREATE TRIGGER IF NOT EXISTS debts_updated AFTER UPDATE ON debts WHEN NEW.updated_at = OLD.updated_at BEGIN UPDATE debts SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                ALTER TABLE debt_payments ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
                UPDATE debt_payments SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');
                CREATE TRIGGER IF NOT EXISTS debt_payments_inserted AFTER INSERT ON debt_payments BEGIN UPDATE debt_payments SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                CREATE TRIGGER IF NOT EXISTS debt_payments_updated AFTER UPDATE ON debt_payments WHEN NEW.updated_at = OLD.updated_at BEGIN UPDATE debt_payments SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = NEW.id; END;
                PRAGMA user_version = 8;
                COMMIT;",
                )
                .map_err(e)?;
            if version > 0 {
                self.set_meta("migrated_from", &version.to_string())?;
            }
        }
        const { assert!(SCHEMA_VERSION == 8, "add a migration step above") };
        Ok(())
    }

    // ---------- loading ----------

    pub fn load(&self) -> Result<AppData> {
        Ok(AppData {
            flows: self.flows()?,
            accounts: self.accounts()?,
            holdings: self.holdings()?,
            goals: self.goals()?,
            buckets: self.buckets()?,
            debts: self.debts()?,
            debt_payments: self.debt_payments()?,
            settings: self.settings()?,
            fred: self.fred_config()?,
            market: self.market_data()?,
        })
    }

    pub fn fred_config(&self) -> Result<FredConfig> {
        Ok(FredConfig {
            api_key: self.setting("fred_api_key")?.unwrap_or_default(),
            sync_on_launch: self.setting("fred_sync_on_launch")?.map(|v| v == "1").unwrap_or(true),
            inflation_source: self
                .setting("fred_inflation_source")?
                .and_then(|v| InflationSource::from_key(&v))
                .unwrap_or_default(),
        })
    }

    pub fn save_fred_config(&self, c: &FredConfig) -> Result<()> {
        self.put_setting("fred_api_key", c.api_key.trim())?;
        self.put_setting("fred_sync_on_launch", if c.sync_on_launch { "1" } else { "0" })?;
        self.put_setting("fred_inflation_source", c.inflation_source.key())
    }

    pub fn market_data(&self) -> Result<MarketData> {
        let mut st = self.conn.prepare("SELECT indicator, value, as_of FROM market_data").map_err(e)?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, String>(2)?)))
            .map_err(e)?;
        let mut values = Vec::new();
        for row in rows {
            let (k, value, as_of) = row.map_err(e)?;
            if let Some(indicator) = Indicator::from_key(&k) {
                values.push(MarketValue { indicator, value, as_of: date_from_sql(&as_of) });
            }
        }
        // Stable display order.
        values.sort_by_key(|v| Indicator::ALL.iter().position(|i| *i == v.indicator));
        Ok(MarketData {
            values,
            last_sync: self.setting("fred_last_sync")?,
            last_error: self.setting("fred_last_error")?.filter(|s| !s.is_empty()),
        })
    }

    /// Store the result of a sync. Values that failed to fetch keep their previous cache.
    pub fn save_market_data(&mut self, values: &[MarketValue], synced_at: &str, error: Option<&str>) -> Result<()> {
        let tx = self.conn.transaction().map_err(e)?;
        for v in values {
            tx.execute(
                "INSERT INTO market_data (indicator, value, as_of) VALUES (?1, ?2, ?3)
                 ON CONFLICT(indicator) DO UPDATE SET value = excluded.value, as_of = excluded.as_of",
                params![v.indicator.key(), v.value, date_to_sql(v.as_of)],
            )
            .map_err(e)?;
        }
        tx.commit().map_err(e)?;
        self.put_setting("fred_last_sync", synced_at)?;
        self.put_setting("fred_last_error", error.unwrap_or(""))
    }

    fn put_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(e)?;
        Ok(())
    }

    fn flow_from_row(r: &Row) -> rusqlite::Result<CashFlow> {
        let kind: String = r.get("kind")?;
        let freq: String = r.get("frequency")?;
        let start: String = r.get("start_date")?;
        let end: Option<String> = r.get("end_date")?;
        Ok(CashFlow {
            id: r.get("id")?,
            kind: FlowKind::from_key(&kind).unwrap_or(FlowKind::Expense),
            name: r.get("name")?,
            category: r.get("category")?,
            amount: r.get("amount")?,
            frequency: Frequency::from_key(&freq).unwrap_or(Frequency::Monthly),
            start_date: date_from_sql(&start),
            end_date: end.as_deref().filter(|s| !s.is_empty()).map(date_from_sql),
            annual_change_pct: r.get("annual_change_pct")?,
            autopay: r.get::<_, i64>("autopay")? != 0,
            notes: r.get("notes")?,
            account_id: r.get("account_id")?,
            bucket_id: r.get("bucket_id")?,
        })
    }

    pub fn flows(&self) -> Result<Vec<CashFlow>> {
        let mut st = self
            .conn
            .prepare("SELECT * FROM cash_flows ORDER BY kind, name COLLATE NOCASE")
            .map_err(e)?;
        let rows = st.query_map([], Self::flow_from_row).map_err(e)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(e)
    }

    pub fn accounts(&self) -> Result<Vec<Account>> {
        let mut st = self.conn.prepare("SELECT * FROM accounts ORDER BY id").map_err(e)?;
        let rows = st
            .query_map([], |r| {
                let kind: String = r.get("kind")?;
                Ok(Account {
                    id: r.get("id")?,
                    name: r.get("name")?,
                    kind: AccountKind::from_key(&kind).unwrap_or(AccountKind::Other),
                    balance: r.get("balance")?,
                    annual_return_pct: r.get("annual_return_pct")?,
                    monthly_contribution: r.get("monthly_contribution")?,
                    contribution_from_cash: r.get::<_, i64>("contribution_from_cash")? != 0,
                    employer_match_monthly: r.get("employer_match_monthly")?,
                    notes: r.get("notes")?,
                    purpose: r.get("purpose")?,
                    minimum_balance: r.get("minimum_balance")?,
                })
            })
            .map_err(e)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(e)
    }

    pub fn holdings(&self) -> Result<Vec<Holding>> {
        let mut st = self
            .conn
            .prepare("SELECT * FROM holdings ORDER BY account_id, symbol")
            .map_err(e)?;
        let rows = st
            .query_map([], |r| {
                let upd: Option<String> = r.get("price_updated")?;
                Ok(Holding {
                    id: r.get("id")?,
                    account_id: r.get("account_id")?,
                    symbol: r.get("symbol")?,
                    shares: r.get("shares")?,
                    price: r.get("price")?,
                    price_updated: upd.as_deref().map(date_from_sql),
                })
            })
            .map_err(e)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(e)
    }

    fn setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .map_err(e)
    }

    pub fn settings(&self) -> Result<Settings> {
        let d = Settings::default();
        Ok(Settings {
            projection_years: self
                .setting("projection_years")?
                .and_then(|v| v.parse().ok())
                .unwrap_or(d.projection_years),
            inflation_pct: self.setting("inflation_pct")?.and_then(|v| v.parse().ok()).unwrap_or(d.inflation_pct),
            currency_symbol: self.setting("currency_symbol")?.unwrap_or(d.currency_symbol),
            dark_mode: self.setting("dark_mode")?.map(|v| v == "1").unwrap_or(d.dark_mode),
            ui_scale: self.setting("ui_scale")?.and_then(|v| UiScale::from_key(&v)).unwrap_or(d.ui_scale),
            natural_scroll: self.setting("natural_scroll")?.map(|v| v == "1").unwrap_or(d.natural_scroll),
            debt_strategy: self.setting("debt_strategy")?.and_then(|v| Strategy::from_key(&v)).unwrap_or(d.debt_strategy),
            debt_extra_monthly: self.setting("debt_extra_monthly")?.and_then(|v| v.parse().ok()).unwrap_or(d.debt_extra_monthly),
        })
    }

    // ---------- saving ----------

    /// Insert (id == 0) or update a cash flow. Returns its id.
    pub fn save_flow(&self, f: &CashFlow) -> Result<i64> {
        let end = f.end_date.map(date_to_sql);
        if f.id == 0 {
            self.conn
                .execute(
                    "INSERT INTO cash_flows (kind, name, category, amount, frequency, start_date, end_date,
                     annual_change_pct, autopay, notes, account_id, bucket_id)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                    params![
                        f.kind.key(),
                        f.name,
                        f.category,
                        f.amount,
                        f.frequency.key(),
                        date_to_sql(f.start_date),
                        end,
                        f.annual_change_pct,
                        f.autopay as i64,
                        f.notes,
                        f.account_id,
                        f.bucket_id
                    ],
                )
                .map_err(e)?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn
                .execute(
                    "UPDATE cash_flows SET kind=?1, name=?2, category=?3, amount=?4, frequency=?5, start_date=?6,
                     end_date=?7, annual_change_pct=?8, autopay=?9, notes=?10, account_id=?11, bucket_id=?12
                     WHERE id=?13",
                    params![
                        f.kind.key(),
                        f.name,
                        f.category,
                        f.amount,
                        f.frequency.key(),
                        date_to_sql(f.start_date),
                        end,
                        f.annual_change_pct,
                        f.autopay as i64,
                        f.notes,
                        f.account_id,
                        f.bucket_id,
                        f.id
                    ],
                )
                .map_err(e)?;
            Ok(f.id)
        }
    }

    pub fn delete_flow(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM cash_flows WHERE id=?1", [id]).map_err(e)?;
        Ok(())
    }

    pub fn save_account(&self, a: &Account) -> Result<i64> {
        if a.id == 0 {
            self.conn
                .execute(
                    "INSERT INTO accounts (name, kind, balance, annual_return_pct, monthly_contribution,
                     contribution_from_cash, employer_match_monthly, notes, purpose, minimum_balance)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                    params![
                        a.name,
                        a.kind.key(),
                        a.balance,
                        a.annual_return_pct,
                        a.monthly_contribution,
                        a.contribution_from_cash as i64,
                        a.employer_match_monthly,
                        a.notes,
                        a.purpose,
                        a.minimum_balance
                    ],
                )
                .map_err(e)?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn
                .execute(
                    "UPDATE accounts SET name=?1, kind=?2, balance=?3, annual_return_pct=?4, monthly_contribution=?5,
                     contribution_from_cash=?6, employer_match_monthly=?7, notes=?8, purpose=?9, minimum_balance=?10
                     WHERE id=?11",
                    params![
                        a.name,
                        a.kind.key(),
                        a.balance,
                        a.annual_return_pct,
                        a.monthly_contribution,
                        a.contribution_from_cash as i64,
                        a.employer_match_monthly,
                        a.notes,
                        a.purpose,
                        a.minimum_balance,
                        a.id
                    ],
                )
                .map_err(e)?;
            Ok(a.id)
        }
    }

    pub fn delete_account(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM accounts WHERE id=?1", [id]).map_err(e)?;
        Ok(())
    }

    pub fn save_holding(&self, h: &Holding) -> Result<i64> {
        let upd = h.price_updated.map(date_to_sql);
        if h.id == 0 {
            self.conn
                .execute(
                    "INSERT INTO holdings (account_id, symbol, shares, price, price_updated) VALUES (?1,?2,?3,?4,?5)",
                    params![h.account_id, h.symbol, h.shares, h.price, upd],
                )
                .map_err(e)?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn
                .execute(
                    "UPDATE holdings SET account_id=?1, symbol=?2, shares=?3, price=?4, price_updated=?5 WHERE id=?6",
                    params![h.account_id, h.symbol, h.shares, h.price, upd, h.id],
                )
                .map_err(e)?;
            Ok(h.id)
        }
    }

    pub fn delete_holding(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM holdings WHERE id=?1", [id]).map_err(e)?;
        Ok(())
    }

    pub fn debts(&self) -> Result<Vec<Debt>> {
        let mut st = self.conn.prepare("SELECT * FROM debts ORDER BY id").map_err(e)?;
        let rows = st
            .query_map([], |r| {
                let kind: String = r.get("kind")?;
                Ok(Debt {
                    id: r.get("id")?,
                    name: r.get("name")?,
                    kind: DebtKind::from_key(&kind).unwrap_or(DebtKind::Other),
                    balance: r.get("balance")?,
                    apr: r.get("apr")?,
                    min_payment: r.get("min_payment")?,
                    account_id: r.get("account_id")?,
                    notes: r.get("notes")?,
                })
            })
            .map_err(e)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(e)
    }

    pub fn save_debt(&self, d: &Debt) -> Result<i64> {
        if d.id == 0 {
            self.conn
                .execute(
                    "INSERT INTO debts (name, kind, balance, apr, min_payment, account_id, notes) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    params![d.name, d.kind.key(), d.balance, d.apr, d.min_payment, d.account_id, d.notes],
                )
                .map_err(e)?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn
                .execute(
                    "UPDATE debts SET name=?1, kind=?2, balance=?3, apr=?4, min_payment=?5, account_id=?6, notes=?7 WHERE id=?8",
                    params![d.name, d.kind.key(), d.balance, d.apr, d.min_payment, d.account_id, d.notes, d.id],
                )
                .map_err(e)?;
            Ok(d.id)
        }
    }

    pub fn delete_debt(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM debts WHERE id=?1", [id]).map_err(e)?;
        Ok(())
    }

    pub fn debt_payments(&self) -> Result<Vec<DebtPayment>> {
        let mut st = self.conn.prepare("SELECT * FROM debt_payments ORDER BY date DESC, id DESC").map_err(e)?;
        let rows = st
            .query_map([], |r| {
                let date: String = r.get("date")?;
                Ok(DebtPayment {
                    id: r.get("id")?,
                    debt_id: r.get("debt_id")?,
                    date: date_from_sql(&date),
                    amount: r.get("amount")?,
                    account_id: r.get("account_id")?,
                    notes: r.get("notes")?,
                    logged: r.get::<_, i64>("logged")? != 0,
                    applied: r.get("applied")?,
                    deducted: r.get::<_, i64>("deducted")? != 0,
                })
            })
            .map_err(e)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(e)
    }

    /// Store a payment row as-is (no balance changes). Used for planned payments and restores.
    fn insert_debt_payment(&self, p: &DebtPayment) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO debt_payments (debt_id, date, amount, account_id, notes, logged, applied, deducted)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    p.debt_id,
                    date_to_sql(p.date),
                    p.amount,
                    p.account_id,
                    p.notes,
                    p.logged as i64,
                    p.applied,
                    p.deducted as i64
                ],
            )
            .map_err(e)?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Plan a future one-time payment (applied by the payoff plan and projections).
    pub fn plan_debt_payment(&self, p: &DebtPayment) -> Result<i64> {
        self.insert_debt_payment(&DebtPayment { id: 0, logged: false, applied: 0.0, deducted: false, ..p.clone() })
    }

    /// Record a payment that was made: lowers the debt balance (never below zero) and, when
    /// `deduct` is set, the paying account's balance. A planned payment (`p.id != 0`) is
    /// turned into a logged one.
    pub fn log_debt_payment(&mut self, p: &DebtPayment, deduct: bool) -> Result<i64> {
        let data = self.load()?;
        let debt = data.debts.iter().find(|d| d.id == p.debt_id).ok_or("That debt no longer exists.")?;
        let account = p.account_id.or(debt.account_id).filter(|id| data.accounts.iter().any(|a| a.id == *id));
        let account = account.or_else(|| data.primary_cash_account().map(|a| a.id));
        let applied = p.amount.max(0.0).min(debt.balance.max(0.0));
        let tx = self.conn.transaction().map_err(e)?;
        tx.execute("UPDATE debts SET balance = MAX(balance - ?1, 0) WHERE id = ?2", params![applied, p.debt_id]).map_err(e)?;
        let deducted = deduct && account.is_some();
        if deducted {
            tx.execute("UPDATE accounts SET balance = balance - ?1 WHERE id = ?2", params![p.amount, account]).map_err(e)?;
        }
        if p.id != 0 {
            tx.execute("DELETE FROM debt_payments WHERE id = ?1", [p.id]).map_err(e)?;
        }
        tx.execute(
            "INSERT INTO debt_payments (debt_id, date, amount, account_id, notes, logged, applied, deducted)
             VALUES (?1,?2,?3,?4,?5,1,?6,?7)",
            params![p.debt_id, date_to_sql(p.date), p.amount, account, p.notes, applied, deducted as i64],
        )
        .map_err(e)?;
        let id = tx.last_insert_rowid();
        tx.commit().map_err(e)?;
        Ok(id)
    }

    /// Delete a payment. For a logged one this is an undo: the debt balance (and the
    /// account balance, if it was deducted) are restored.
    pub fn delete_debt_payment(&mut self, id: i64) -> Result<()> {
        let Some(p) = self.debt_payments()?.into_iter().find(|p| p.id == id) else { return Ok(()) };
        let tx = self.conn.transaction().map_err(e)?;
        if p.logged {
            tx.execute("UPDATE debts SET balance = balance + ?1 WHERE id = ?2", params![p.applied, p.debt_id]).map_err(e)?;
            if p.deducted && p.account_id.is_some() {
                tx.execute("UPDATE accounts SET balance = balance + ?1 WHERE id = ?2", params![p.amount, p.account_id])
                    .map_err(e)?;
            }
        }
        tx.execute("DELETE FROM debt_payments WHERE id = ?1", [id]).map_err(e)?;
        tx.commit().map_err(e)
    }

    pub fn buckets(&self) -> Result<Vec<Bucket>> {
        let mut st = self.conn.prepare("SELECT * FROM buckets ORDER BY account_id, id").map_err(e)?;
        let rows = st
            .query_map([], |r| {
                Ok(Bucket {
                    id: r.get("id")?,
                    account_id: r.get("account_id")?,
                    name: r.get("name")?,
                    balance: r.get("balance")?,
                    monthly_add: r.get("monthly_add")?,
                    target: r.get("target")?,
                    notes: r.get("notes")?,
                    debt_id: r.get("debt_id")?,
                })
            })
            .map_err(e)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(e)
    }

    pub fn save_bucket(&self, b: &Bucket) -> Result<i64> {
        if b.id == 0 {
            self.conn
                .execute(
                    "INSERT INTO buckets (account_id, name, balance, monthly_add, target, notes, debt_id)
                     VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    params![b.account_id, b.name, b.balance, b.monthly_add, b.target, b.notes, b.debt_id],
                )
                .map_err(e)?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn
                .execute(
                    "UPDATE buckets SET account_id=?1, name=?2, balance=?3, monthly_add=?4, target=?5, notes=?6, debt_id=?7
                     WHERE id=?8",
                    params![b.account_id, b.name, b.balance, b.monthly_add, b.target, b.notes, b.debt_id, b.id],
                )
                .map_err(e)?;
            Ok(b.id)
        }
    }

    /// Give a debt its payment bucket (in the account that pays it), if it doesn't have one.
    pub fn add_debt_bucket(&mut self, debt_id: i64) -> Result<()> {
        let exists: bool = self
            .conn
            .query_row("SELECT EXISTS(SELECT 1 FROM buckets WHERE debt_id = ?1)", [debt_id], |r| r.get(0))
            .map_err(e)?;
        if !exists {
            let data = self.load()?;
            let Some(debt) = data.debts.iter().find(|d| d.id == debt_id) else { return Ok(()) };
            let Some(account) = debt_account(&data, debt) else { return Err("Add a checking account first.".into()) };
            self.save_bucket(&Bucket {
                id: 0,
                account_id: account,
                name: format!("{} payment", debt.name),
                balance: 0.0,
                monthly_add: 0.0,
                target: None,
                notes: String::new(),
                debt_id: Some(debt_id),
            })?;
        }
        self.sync_debt_buckets()
    }

    /// Stop setting aside a debt's payments (removes its bucket; the debt plan continues).
    pub fn remove_debt_bucket(&self, debt_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM buckets WHERE debt_id = ?1", [debt_id]).map_err(e)?;
        Ok(())
    }

    /// Keep debt payment buckets in step with the payoff plan: each one sits in the account
    /// that pays its debt and holds that debt's planned payment for this month. Called after
    /// every change, so balances, the extra amount and the strategy are always reflected.
    pub fn sync_debt_buckets(&mut self) -> Result<()> {
        let data = self.load()?;
        if !data.buckets.iter().any(|b| b.debt_id.is_some()) {
            return Ok(());
        }
        let this_month = crate::debt::Simulator::new(
            &data.debts,
            data.settings.debt_strategy,
            data.settings.debt_extra_monthly,
            true,
        )
        .step();
        for b in data.buckets.iter().filter(|b| b.debt_id.is_some()) {
            let Some(i) = data.debts.iter().position(|d| Some(d.id) == b.debt_id) else { continue };
            let Some(account) = debt_account(&data, &data.debts[i]) else { continue };
            let want = Bucket {
                account_id: account,
                name: format!("{} payment", data.debts[i].name),
                balance: (this_month.payments[i] * 100.0).round() / 100.0,
                monthly_add: 0.0,
                target: None,
                ..b.clone()
            };
            if want != *b {
                self.save_bucket(&want)?;
            }
        }
        Ok(())
    }

    /// Deleting a bucket moves bills paid from it back to its account.
    pub fn delete_bucket(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM buckets WHERE id=?1", [id]).map_err(e)?;
        Ok(())
    }

    pub fn goals(&self) -> Result<Vec<Goal>> {
        let mut st = self.conn.prepare("SELECT * FROM goals ORDER BY priority, id").map_err(e)?;
        let rows = st
            .query_map([], |r| {
                let target: Option<String> = r.get("target_date")?;
                let bought: Option<String> = r.get("purchased_on")?;
                Ok(Goal {
                    id: r.get("id")?,
                    name: r.get("name")?,
                    cost: r.get("cost")?,
                    target_date: target.as_deref().filter(|s| !s.is_empty()).map(date_from_sql),
                    priority: r.get("priority")?,
                    notes: r.get("notes")?,
                    purchased_on: bought.as_deref().filter(|s| !s.is_empty()).map(date_from_sql),
                })
            })
            .map_err(e)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(e)
    }

    /// Insert (id == 0, appended after the last goal) or update a goal.
    pub fn save_goal(&self, g: &Goal) -> Result<i64> {
        let target = g.target_date.map(date_to_sql);
        let bought = g.purchased_on.map(date_to_sql);
        if g.id == 0 {
            let next: i64 = self
                .conn
                .query_row("SELECT COALESCE(MAX(priority), 0) + 1 FROM goals", [], |r| r.get(0))
                .map_err(e)?;
            self.conn
                .execute(
                    "INSERT INTO goals (name, cost, target_date, priority, notes, purchased_on) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![g.name, g.cost, target, if g.priority > 0 { g.priority } else { next }, g.notes, bought],
                )
                .map_err(e)?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn
                .execute(
                    "UPDATE goals SET name=?1, cost=?2, target_date=?3, priority=?4, notes=?5, purchased_on=?6 WHERE id=?7",
                    params![g.name, g.cost, target, g.priority, g.notes, bought, g.id],
                )
                .map_err(e)?;
            Ok(g.id)
        }
    }

    pub fn delete_goal(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM goals WHERE id=?1", [id]).map_err(e)?;
        Ok(())
    }

    /// Swap a goal's priority with its neighbour (-1 = up, +1 = down) among open goals.
    pub fn move_goal(&mut self, id: i64, direction: i32) -> Result<()> {
        let open: Vec<Goal> = self.goals()?.into_iter().filter(|g| g.purchased_on.is_none()).collect();
        let Some(i) = open.iter().position(|g| g.id == id) else { return Ok(()) };
        let j = i as i64 + direction as i64;
        if j < 0 || j as usize >= open.len() {
            return Ok(());
        }
        let tx = self.conn.transaction().map_err(e)?;
        // Renumber densely in the new order so equal priorities can't get stuck.
        let mut order: Vec<i64> = open.iter().map(|g| g.id).collect();
        order.swap(i, j as usize);
        for (n, gid) in order.iter().enumerate() {
            tx.execute("UPDATE goals SET priority=?1 WHERE id=?2", params![n as i64 + 1, gid]).map_err(e)?;
        }
        tx.commit().map_err(e)
    }

    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        let pairs = [
            ("projection_years", s.projection_years.to_string()),
            ("inflation_pct", s.inflation_pct.to_string()),
            ("currency_symbol", s.currency_symbol.clone()),
            ("dark_mode", if s.dark_mode { "1".into() } else { "0".into() }),
            ("ui_scale", s.ui_scale.key().to_string()),
            ("natural_scroll", if s.natural_scroll { "1".into() } else { "0".into() }),
            ("debt_strategy", s.debt_strategy.key().to_string()),
            ("debt_extra_monthly", s.debt_extra_monthly.to_string()),
        ];
        for (k, v) in pairs {
            self.conn
                .execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![k, v],
                )
                .map_err(e)?;
        }
        Ok(())
    }

    /// Replace all data (used by JSON import and sample data).
    pub fn replace_all(&mut self, data: &AppData) -> Result<()> {
        let tx = self.conn.transaction().map_err(e)?;
        tx.execute_batch(
            "DELETE FROM cash_flows; DELETE FROM debt_payments; DELETE FROM buckets; DELETE FROM debts; DELETE FROM holdings; DELETE FROM accounts; DELETE FROM goals;",
        )
        .map_err(e)?;
        tx.commit().map_err(e)?;
        // Accounts get new ids; remap holdings onto them.
        let mut id_map = std::collections::HashMap::new();
        for a in &data.accounts {
            let new_id = self.save_account(&Account { id: 0, ..a.clone() })?;
            id_map.insert(a.id, new_id);
        }
        for h in &data.holdings {
            if let Some(&acc) = id_map.get(&h.account_id) {
                self.save_holding(&Holding { id: 0, account_id: acc, ..h.clone() })?;
            }
        }
        // Debts, buckets and flows reference each other by id; map them to the new ids.
        let mut debt_map = std::collections::HashMap::new();
        for d in &data.debts {
            let new_id =
                self.save_debt(&Debt { id: 0, account_id: d.account_id.and_then(|a| id_map.get(&a).copied()), ..d.clone() })?;
            debt_map.insert(d.id, new_id);
        }
        // Payment history is restored as-is (backed-up balances already include it).
        for p in &data.debt_payments {
            if let Some(&debt_id) = debt_map.get(&p.debt_id) {
                self.insert_debt_payment(&DebtPayment {
                    id: 0,
                    debt_id,
                    account_id: p.account_id.and_then(|a| id_map.get(&a).copied()),
                    ..p.clone()
                })?;
            }
        }
        let mut bucket_map = std::collections::HashMap::new();
        for b in &data.buckets {
            let debt_id = match b.debt_id {
                Some(old) => match debt_map.get(&old) {
                    Some(&new) => Some(new),
                    None => continue, // its debt is gone
                },
                None => None,
            };
            if let Some(&acc) = id_map.get(&b.account_id) {
                let new_id = self.save_bucket(&Bucket { id: 0, account_id: acc, debt_id, ..b.clone() })?;
                bucket_map.insert(b.id, new_id);
            }
        }
        for f in &data.flows {
            self.save_flow(&CashFlow {
                id: 0,
                account_id: f.account_id.and_then(|a| id_map.get(&a).copied()),
                bucket_id: f.bucket_id.and_then(|b| bucket_map.get(&b).copied()),
                ..f.clone()
            })?;
        }
        for g in &data.goals {
            self.save_goal(&Goal { id: 0, ..g.clone() })?;
        }
        self.save_settings(&data.settings)?;
        self.sync_debt_buckets()
    }
}

/// The account that pays a debt: its chosen account, else the primary cash account.
fn debt_account(data: &AppData, debt: &Debt) -> Option<i64> {
    debt.account_id
        .filter(|id| data.accounts.iter().any(|a| a.id == *id))
        .or_else(|| data.primary_cash_account().map(|a| a.id))
}

/// A backup file containing all data as JSON.
pub fn export_json(data: &AppData, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(e)?;
    }
    let json = serde_json::to_string_pretty(data).map_err(e)?;
    std::fs::write(path, json).map_err(e)
}

pub fn import_json(path: &Path) -> Result<AppData> {
    let s = std::fs::read_to_string(path).map_err(e)?;
    serde_json::from_str(&s).map_err(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    #[test]
    fn upgrading_a_1x_file_keeps_a_copy_and_stamps_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("penguinfin.db");
        std::fs::copy(fixture("schema-v7.db"), &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let (db, copy) = Db::open_upgrading(&path).unwrap();
        let copy = copy.expect("a copy is made before upgrading");
        assert!(copy.file_name().unwrap().to_str().unwrap().starts_with("penguinfin-1.x-backup-"));
        let c = Connection::open(&copy).unwrap();
        assert_eq!(c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), 7);
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(db.meta("migrated_from").unwrap().as_deref(), Some("7"));
        assert!(db.meta("created_at").unwrap().is_some());
        let stamped: i64 = db.conn.query_row("SELECT count(*) FROM accounts WHERE updated_at != ''", [], |r| r.get(0)).unwrap();
        assert_eq!(stamped as usize, db.accounts().unwrap().len());
        // Same data as before the upgrade.
        let old = Db::open_copy(&copy).unwrap().load().unwrap();
        assert_eq!(old.accounts, db.load().unwrap().accounts);
        assert_eq!(old.debt_payments, db.load().unwrap().debt_payments);
        drop(db);
        assert_ne!(std::fs::read(&path).unwrap(), before);
        // A second open has nothing to upgrade and makes no copy.
        assert!(Db::open_upgrading(&path).unwrap().1.is_none());
    }

    #[test]
    fn new_rows_and_edits_get_updated_at() {
        let db = Db::open_in_memory().unwrap();
        let mut a = Account {
            id: 0,
            name: "Checking".into(),
            kind: AccountKind::Checking,
            balance: 10.0,
            annual_return_pct: 0.0,
            monthly_contribution: 0.0,
            contribution_from_cash: false,
            employer_match_monthly: 0.0,
            notes: String::new(),
            purpose: String::new(),
            minimum_balance: 0.0,
        };
        a.id = db.save_account(&a).unwrap();
        let stamp = |db: &Db| -> String {
            db.conn.query_row("SELECT updated_at FROM accounts WHERE id = ?1", [a.id], |r| r.get(0)).unwrap()
        };
        assert!(stamp(&db).ends_with('Z'));
        db.conn.execute("UPDATE accounts SET updated_at = 'old' WHERE id = ?1", [a.id]).unwrap();
        a.balance = 20.0;
        db.save_account(&a).unwrap();
        assert_ne!(stamp(&db), "old");
    }

    #[test]
    fn a_new_database_has_no_migrated_from() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.meta("migrated_from").unwrap(), None);
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn roundtrip_everything() {
        let mut db = Db::open_in_memory().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        let mut data = crate::sample::sample_data(today);
        data.settings.ui_scale = UiScale::Large;
        data.settings.natural_scroll = true;
        data.settings.dark_mode = true;
        db.replace_all(&data).unwrap();
        let loaded = db.load().unwrap();
        assert_eq!(loaded.flows.len(), data.flows.len());
        assert_eq!(loaded.accounts.len(), data.accounts.len());
        assert_eq!(loaded.holdings.len(), data.holdings.len());
        assert_eq!(loaded.debts.len(), data.debts.len());
        assert_eq!(loaded.debts[0].apr, data.debts[0].apr);
        assert_eq!(loaded.settings, data.settings);
        assert!((loaded.net_worth() - data.net_worth()).abs() < 1e-6);

        // Update + delete.
        let mut f = loaded.flows[0].clone();
        f.amount = 123.45;
        f.end_date = Some(today);
        db.save_flow(&f).unwrap();
        let again = db.load().unwrap();
        let g = again.flows.iter().find(|x| x.id == f.id).unwrap();
        assert_eq!(g.amount, 123.45);
        assert_eq!(g.end_date, Some(today));
        db.delete_flow(f.id).unwrap();
        assert_eq!(db.flows().unwrap().len(), data.flows.len() - 1);

        // Deleting an account cascades to its holdings.
        let acc = again.holdings[0].account_id;
        db.delete_account(acc).unwrap();
        assert!(db.holdings().unwrap().iter().all(|h| h.account_id != acc));
    }

    #[test]
    fn fred_config_and_market_cache() {
        let mut db = Db::open_in_memory().unwrap();
        let c = db.fred_config().unwrap();
        assert!(c.sync_on_launch && c.api_key.is_empty() && c.inflation_source == InflationSource::Manual);
        let cfg = FredConfig { api_key: "k".repeat(32), sync_on_launch: false, inflation_source: InflationSource::CpiInflation };
        db.save_fred_config(&cfg).unwrap();
        assert_eq!(db.fred_config().unwrap(), cfg);

        let d = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let v = |i, x| MarketValue { indicator: i, value: x, as_of: d };
        db.save_market_data(&[v(Indicator::FedFunds, 4.1), v(Indicator::CpiInflation, 2.9)], "2026-11-15 09:00", None).unwrap();
        // A later partial sync updates one value and keeps the other cached.
        db.save_market_data(&[v(Indicator::FedFunds, 3.9)], "2026-11-16 09:00", Some("timeout")).unwrap();
        let m = db.market_data().unwrap();
        assert_eq!(m.get(Indicator::FedFunds).unwrap().value, 3.9);
        assert_eq!(m.get(Indicator::CpiInflation).unwrap().value, 2.9);
        assert_eq!(m.last_sync.as_deref(), Some("2026-11-16 09:00"));
        assert_eq!(m.last_error.as_deref(), Some("timeout"));
        // CPI comes before fed funds in display order.
        assert_eq!(m.values[0].indicator, Indicator::CpiInflation);

        // Replacing all data (backup restore) keeps the FRED key.
        db.replace_all(&AppData::default()).unwrap();
        assert_eq!(db.fred_config().unwrap().api_key, "k".repeat(32));
    }

    #[test]
    fn goals_crud_order_and_backup() {
        let mut db = Db::open_in_memory().unwrap();
        let g = |name: &str, cost| Goal {
            id: 0,
            name: name.into(),
            cost,
            target_date: None,
            priority: 0,
            notes: String::new(),
            purchased_on: None,
        };
        let a = db.save_goal(&g("Laptop", 1500.0)).unwrap();
        let b = db.save_goal(&g("Bike", 800.0)).unwrap();
        let c = db.save_goal(&g("Trip", 3000.0)).unwrap();
        let names = |db: &Db| db.goals().unwrap().into_iter().map(|g| g.name).collect::<Vec<_>>();
        assert_eq!(names(&db), ["Laptop", "Bike", "Trip"]);
        db.move_goal(c, -1).unwrap();
        assert_eq!(names(&db), ["Laptop", "Trip", "Bike"]);
        db.move_goal(a, -1).unwrap(); // already first: no-op
        assert_eq!(names(&db), ["Laptop", "Trip", "Bike"]);

        let mut bike = db.goals().unwrap().into_iter().find(|x| x.id == b).unwrap();
        bike.purchased_on = NaiveDate::from_ymd_opt(2026, 11, 15);
        bike.target_date = NaiveDate::from_ymd_opt(2027, 1, 1);
        db.save_goal(&bike).unwrap();
        let loaded = db.load().unwrap();
        assert_eq!(loaded.goals.iter().find(|x| x.id == b).unwrap(), &bike);

        // Backups include goals; restore brings them back.
        let mut restored = Db::open_in_memory().unwrap();
        restored.replace_all(&loaded).unwrap();
        assert_eq!(names(&restored), ["Laptop", "Trip", "Bike"]);
        db.delete_goal(a).unwrap();
        assert_eq!(names(&db), ["Trip", "Bike"]);
    }

    #[test]
    fn account_purpose_and_minimum_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        let id = db
            .save_account(&Account {
                id: 0,
                name: "Medical fund".into(),
                kind: AccountKind::Savings,
                balance: 2500.0,
                annual_return_pct: 4.0,
                monthly_contribution: 0.0,
                contribution_from_cash: true,
                employer_match_monthly: 0.0,
                notes: String::new(),
                purpose: "Medical".into(),
                minimum_balance: 2000.0,
            })
            .unwrap();
        let a = db.accounts().unwrap().into_iter().find(|a| a.id == id).unwrap();
        assert_eq!((a.purpose.as_str(), a.minimum_balance), ("Medical", 2000.0));
        // Old backups without the new fields still load.
        let old: Account = serde_json::from_str(r#"{"id":1,"name":"x","kind":"Checking","balance":1.0,"annual_return_pct":0.0,
            "monthly_contribution":0.0,"contribution_from_cash":false,"employer_match_monthly":0.0,"notes":""}"#).unwrap();
        assert_eq!((old.purpose.as_str(), old.minimum_balance), ("", 0.0));
    }

    #[test]
    fn buckets_and_flow_links_survive_restore_and_deletes() {
        let mut db = Db::open_in_memory().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        db.replace_all(&crate::sample::sample_data(today)).unwrap();
        let data = db.load().unwrap();
        assert!(!data.buckets.is_empty(), "sample has buckets");
        let linked = data.flows.iter().find(|f| f.bucket_id.is_some()).unwrap().clone();
        let bucket = data.bucket(linked.bucket_id.unwrap()).unwrap().clone();

        // Restore into a fresh DB: ids change, links still point to the same names.
        let mut other = Db::open_in_memory().unwrap();
        other.replace_all(&data).unwrap();
        let back = other.load().unwrap();
        let f = back.flows.iter().find(|f| f.name == linked.name).unwrap();
        let b = back.bucket(f.bucket_id.unwrap()).unwrap();
        assert_eq!(b.name, bucket.name);
        assert_eq!(back.account_name(b.account_id), data.account_name(bucket.account_id));

        // Deleting the bucket unlinks the bill; deleting the account removes its buckets.
        db.delete_bucket(bucket.id).unwrap();
        assert_eq!(db.flows().unwrap().iter().find(|x| x.id == linked.id).unwrap().bucket_id, None);
        db.delete_account(bucket.account_id).unwrap();
        assert!(db.buckets().unwrap().iter().all(|b| b.account_id != bucket.account_id));
    }

    #[test]
    fn debt_payment_buckets_follow_the_plan() {
        let mut db = Db::open_in_memory().unwrap();
        let checking = db
            .save_account(&Account {
                id: 0,
                name: "Checking".into(),
                kind: AccountKind::Checking,
                balance: 3000.0,
                annual_return_pct: 0.0,
                monthly_contribution: 0.0,
                contribution_from_cash: false,
                employer_match_monthly: 0.0,
                notes: String::new(),
                purpose: String::new(),
                minimum_balance: 0.0,
            })
            .unwrap();
        let mk = |name: &str, balance, apr, min| Debt {
            id: 0,
            name: name.into(),
            kind: DebtKind::CreditCard,
            balance,
            apr,
            min_payment: min,
            account_id: None,
            notes: String::new(),
        };
        let visa = db.save_debt(&mk("Visa", 2000.0, 24.0, 60.0)).unwrap();
        let store_card = db.save_debt(&mk("Store", 500.0, 12.0, 25.0)).unwrap();
        db.add_debt_bucket(visa).unwrap();
        db.add_debt_bucket(store_card).unwrap();
        db.add_debt_bucket(visa).unwrap(); // no duplicate
        let b = |db: &Db, id| db.buckets().unwrap().into_iter().find(|b| b.debt_id == Some(id)).unwrap();
        assert_eq!(db.buckets().unwrap().len(), 2);
        assert_eq!(b(&db, visa).account_id, checking);
        assert_eq!(b(&db, visa).name, "Visa payment");
        assert_eq!(b(&db, visa).balance, 60.0);

        // Extra goes to the highest APR (Visa) → its bucket grows.
        let mut s = db.settings().unwrap();
        s.debt_extra_monthly = 200.0;
        db.save_settings(&s).unwrap();
        db.sync_debt_buckets().unwrap();
        assert_eq!(b(&db, visa).balance, 260.0);
        assert_eq!(b(&db, store_card).balance, 25.0);
        // Snowball moves the extra to the smaller balance.
        s.debt_strategy = Strategy::Snowball;
        db.save_settings(&s).unwrap();
        db.sync_debt_buckets().unwrap();
        assert_eq!(b(&db, visa).balance, 60.0);
        assert_eq!(b(&db, store_card).balance, 225.0);

        // Restore keeps the link; deleting the debt removes its bucket.
        let data = db.load().unwrap();
        let mut other = Db::open_in_memory().unwrap();
        other.replace_all(&data).unwrap();
        let restored = other.load().unwrap();
        let rb = restored.buckets.iter().find(|b| b.name == "Visa payment").unwrap();
        assert_eq!(restored.debts.iter().find(|d| Some(d.id) == rb.debt_id).unwrap().name, "Visa");
        db.delete_debt(visa).unwrap();
        assert!(db.buckets().unwrap().iter().all(|b| b.debt_id != Some(visa)));
        db.remove_debt_bucket(store_card).unwrap();
        assert!(db.buckets().unwrap().is_empty());
        assert_eq!(db.debts().unwrap().len(), 1);
    }

    #[test]
    fn logging_and_undoing_one_time_payments() {
        let mut db = Db::open_in_memory().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        db.replace_all(&crate::sample::sample_data(today)).unwrap();
        let data = db.load().unwrap();
        let store_card = data.debts.iter().find(|d| d.name == "Store card").unwrap().clone();
        let checking = data.primary_cash_account().unwrap().clone();
        let pay = |amount, date| DebtPayment {
            id: 0,
            debt_id: store_card.id,
            date,
            amount,
            account_id: None,
            notes: "bonus".into(),
            logged: false,
            applied: 0.0,
            deducted: false,
        };

        // Log $400: debt and checking both go down.
        let id = db.log_debt_payment(&pay(400.0, today), true).unwrap();
        let after = db.load().unwrap();
        let debt_bal = |d: &AppData| d.debts.iter().find(|x| x.id == store_card.id).unwrap().balance;
        let acct_bal = |d: &AppData| d.accounts.iter().find(|a| a.id == checking.id).unwrap().balance;
        assert_eq!(debt_bal(&after), store_card.balance - 400.0);
        assert_eq!(acct_bal(&after), checking.balance - 400.0);
        let logged = after.debt_payments.iter().find(|p| p.id == id).unwrap();
        assert!(logged.logged && logged.deducted && logged.applied == 400.0);
        assert_eq!(logged.account_id, Some(checking.id));

        // Overpaying is capped at what's owed; undo restores exactly.
        let id2 = db.log_debt_payment(&pay(1000.0, today), false).unwrap();
        let after2 = db.load().unwrap();
        assert_eq!(debt_bal(&after2), 0.0);
        assert_eq!(acct_bal(&after2), checking.balance - 400.0); // not deducted this time
        db.delete_debt_payment(id2).unwrap();
        db.delete_debt_payment(id).unwrap();
        let undone = db.load().unwrap();
        assert_eq!(debt_bal(&undone), store_card.balance);
        assert_eq!(acct_bal(&undone), checking.balance);

        // A planned payment changes nothing until it's logged.
        let future = NaiveDate::from_ymd_opt(2027, 4, 15).unwrap();
        let pid = db.plan_debt_payment(&pay(300.0, future)).unwrap();
        assert_eq!(debt_bal(&db.load().unwrap()), store_card.balance);
        let planned = db.load().unwrap().debt_payments.into_iter().find(|p| p.id == pid).unwrap();
        db.log_debt_payment(&planned, true).unwrap();
        let done = db.load().unwrap();
        assert_eq!(debt_bal(&done), store_card.balance - 300.0);
        assert_eq!(done.debt_payments.len(), 1);
        assert!(done.debt_payments[0].logged);

        // Restore keeps history without applying it twice.
        let mut other = Db::open_in_memory().unwrap();
        other.replace_all(&done).unwrap();
        let restored = other.load().unwrap();
        assert_eq!(restored.debt_payments.len(), 1);
        assert_eq!(restored.debts.iter().find(|d| d.name == "Store card").unwrap().balance, store_card.balance - 300.0);
    }

    #[test]
    fn backups_never_contain_the_api_key() {
        let mut data = AppData::default();
        data.fred.api_key = "secretsecretsecretsecretsecret12".into();
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("secretsecret"));
    }

    #[test]
    fn migrates_v1_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                CREATE TABLE cash_flows (id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, name TEXT NOT NULL,
                  category TEXT NOT NULL DEFAULT '', amount REAL NOT NULL, frequency TEXT NOT NULL, start_date TEXT NOT NULL,
                  end_date TEXT, annual_change_pct REAL NOT NULL DEFAULT 0, autopay INTEGER NOT NULL DEFAULT 0, notes TEXT NOT NULL DEFAULT '');
                CREATE TABLE accounts (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, kind TEXT NOT NULL,
                  balance REAL NOT NULL DEFAULT 0, annual_return_pct REAL NOT NULL DEFAULT 0, monthly_contribution REAL NOT NULL DEFAULT 0,
                  contribution_from_cash INTEGER NOT NULL DEFAULT 0, employer_match_monthly REAL NOT NULL DEFAULT 0, notes TEXT NOT NULL DEFAULT '');
                CREATE TABLE holdings (id INTEGER PRIMARY KEY AUTOINCREMENT, account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                  symbol TEXT NOT NULL, shares REAL NOT NULL, price REAL NOT NULL, price_updated TEXT);
                INSERT INTO accounts (name, kind, balance) VALUES ('Checking', 'checking', 100);
                PRAGMA user_version = 1;").unwrap();
        }
        let db = Db::open(&path).unwrap();
        let data = db.load().unwrap();
        assert_eq!(data.accounts.len(), 1);
        assert!(data.market.values.is_empty());
        // v3 columns added with defaults.
        assert_eq!(data.accounts[0].minimum_balance, 0.0);
        assert!(data.goals.is_empty());
    }

    #[test]
    fn file_db_persists_and_json_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/penguinfin.db");
        let today = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        let data = crate::sample::sample_data(today);
        {
            let mut db = Db::open(&path).unwrap();
            db.replace_all(&data).unwrap();
        }
        let db = Db::open(&path).unwrap();
        let loaded = db.load().unwrap();
        assert_eq!(loaded.flows.len(), data.flows.len());

        let json = dir.path().join("backup.json");
        export_json(&loaded, &json).unwrap();
        let back = import_json(&json).unwrap();
        assert_eq!(back, loaded);
    }
}
