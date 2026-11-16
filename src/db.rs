//! Local SQLite storage. Everything lives in a single file in the user's data dir.

use crate::debt::{Debt, DebtKind, Strategy};
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

const SCHEMA_VERSION: i64 = 5;

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
        const { assert!(SCHEMA_VERSION == 5, "add a migration step above") };
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
                })
            })
            .map_err(e)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(e)
    }

    pub fn save_bucket(&self, b: &Bucket) -> Result<i64> {
        if b.id == 0 {
            self.conn
                .execute(
                    "INSERT INTO buckets (account_id, name, balance, monthly_add, target, notes) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![b.account_id, b.name, b.balance, b.monthly_add, b.target, b.notes],
                )
                .map_err(e)?;
            Ok(self.conn.last_insert_rowid())
        } else {
            self.conn
                .execute(
                    "UPDATE buckets SET account_id=?1, name=?2, balance=?3, monthly_add=?4, target=?5, notes=?6 WHERE id=?7",
                    params![b.account_id, b.name, b.balance, b.monthly_add, b.target, b.notes, b.id],
                )
                .map_err(e)?;
            Ok(b.id)
        }
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
            "DELETE FROM cash_flows; DELETE FROM buckets; DELETE FROM debts; DELETE FROM holdings; DELETE FROM accounts; DELETE FROM goals;",
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
        // Buckets and flows reference accounts/buckets by id; map them to the new ids.
        let mut bucket_map = std::collections::HashMap::new();
        for b in &data.buckets {
            if let Some(&acc) = id_map.get(&b.account_id) {
                let new_id = self.save_bucket(&Bucket { id: 0, account_id: acc, ..b.clone() })?;
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
        for d in &data.debts {
            self.save_debt(&Debt { id: 0, account_id: d.account_id.and_then(|a| id_map.get(&a).copied()), ..d.clone() })?;
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
