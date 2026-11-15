//! Local SQLite storage. Everything lives in a single file in the user's data dir.

use crate::model::*;
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

/// `$XDG_DATA_HOME/penguinfin` (inside the flatpak: `~/.var/app/<id>/data/penguinfin`).
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("penguinfin")
}

pub fn default_db_path() -> PathBuf {
    data_dir().join("penguinfin.db")
}

const SCHEMA_VERSION: i64 = 1;

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
        if version < SCHEMA_VERSION {
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
        Ok(())
    }

    // ---------- loading ----------

    pub fn load(&self) -> Result<AppData> {
        Ok(AppData {
            flows: self.flows()?,
            accounts: self.accounts()?,
            holdings: self.holdings()?,
            settings: self.settings()?,
        })
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
                     annual_change_pct, autopay, notes) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
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
                        f.notes
                    ],
                )
                .map_err(e)?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn
                .execute(
                    "UPDATE cash_flows SET kind=?1, name=?2, category=?3, amount=?4, frequency=?5, start_date=?6,
                     end_date=?7, annual_change_pct=?8, autopay=?9, notes=?10 WHERE id=?11",
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
                     contribution_from_cash, employer_match_monthly, notes) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        a.name,
                        a.kind.key(),
                        a.balance,
                        a.annual_return_pct,
                        a.monthly_contribution,
                        a.contribution_from_cash as i64,
                        a.employer_match_monthly,
                        a.notes
                    ],
                )
                .map_err(e)?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn
                .execute(
                    "UPDATE accounts SET name=?1, kind=?2, balance=?3, annual_return_pct=?4, monthly_contribution=?5,
                     contribution_from_cash=?6, employer_match_monthly=?7, notes=?8 WHERE id=?9",
                    params![
                        a.name,
                        a.kind.key(),
                        a.balance,
                        a.annual_return_pct,
                        a.monthly_contribution,
                        a.contribution_from_cash as i64,
                        a.employer_match_monthly,
                        a.notes,
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

    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        let pairs = [
            ("projection_years", s.projection_years.to_string()),
            ("inflation_pct", s.inflation_pct.to_string()),
            ("currency_symbol", s.currency_symbol.clone()),
            ("dark_mode", if s.dark_mode { "1".into() } else { "0".into() }),
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
        tx.execute_batch("DELETE FROM holdings; DELETE FROM accounts; DELETE FROM cash_flows;").map_err(e)?;
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
        for f in &data.flows {
            self.save_flow(&CashFlow { id: 0, ..f.clone() })?;
        }
        self.save_settings(&data.settings)?;
        Ok(())
    }
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

    #[test]
    fn roundtrip_everything() {
        let mut db = Db::open_in_memory().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        let data = crate::sample::sample_data(today);
        db.replace_all(&data).unwrap();
        let loaded = db.load().unwrap();
        assert_eq!(loaded.flows.len(), data.flows.len());
        assert_eq!(loaded.accounts.len(), data.accounts.len());
        assert_eq!(loaded.holdings.len(), data.holdings.len());
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
