//! Core domain types for PenguinFin.

use chrono::{Datelike, Duration, Months, NaiveDate};
use serde::{Deserialize, Serialize};

/// How often a cash flow repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frequency {
    Once,
    Weekly,
    Biweekly,
    SemiMonthly,
    Monthly,
    Quarterly,
    SemiAnnually,
    Annually,
}

impl Frequency {
    pub const ALL: [Frequency; 8] = [
        Frequency::Monthly,
        Frequency::Weekly,
        Frequency::Biweekly,
        Frequency::SemiMonthly,
        Frequency::Quarterly,
        Frequency::SemiAnnually,
        Frequency::Annually,
        Frequency::Once,
    ];

    /// Average number of occurrences per year (0 for one-time items).
    pub fn per_year(self) -> f64 {
        match self {
            Frequency::Once => 0.0,
            Frequency::Weekly => 52.0,
            Frequency::Biweekly => 26.0,
            Frequency::SemiMonthly => 24.0,
            Frequency::Monthly => 12.0,
            Frequency::Quarterly => 4.0,
            Frequency::SemiAnnually => 2.0,
            Frequency::Annually => 1.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Frequency::Once => "One time",
            Frequency::Weekly => "Weekly",
            Frequency::Biweekly => "Every 2 weeks",
            Frequency::SemiMonthly => "Twice a month",
            Frequency::Monthly => "Monthly",
            Frequency::Quarterly => "Quarterly",
            Frequency::SemiAnnually => "Twice a year",
            Frequency::Annually => "Yearly",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Frequency::Once => "once",
            Frequency::Weekly => "weekly",
            Frequency::Biweekly => "biweekly",
            Frequency::SemiMonthly => "semimonthly",
            Frequency::Monthly => "monthly",
            Frequency::Quarterly => "quarterly",
            Frequency::SemiAnnually => "semiannually",
            Frequency::Annually => "annually",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.key() == s)
    }
}

/// Income, a discretionary/recurring expense, or a bill with a due date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowKind {
    Income,
    Expense,
    Bill,
}

impl FlowKind {
    pub fn key(self) -> &'static str {
        match self {
            FlowKind::Income => "income",
            FlowKind::Expense => "expense",
            FlowKind::Bill => "bill",
        }
    }
    pub fn from_key(s: &str) -> Option<Self> {
        match s {
            "income" => Some(FlowKind::Income),
            "expense" => Some(FlowKind::Expense),
            "bill" => Some(FlowKind::Bill),
            _ => None,
        }
    }
    pub fn is_outflow(self) -> bool {
        !matches!(self, FlowKind::Income)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CashFlow {
    pub id: i64,
    pub kind: FlowKind,
    pub name: String,
    pub category: String,
    /// Amount per occurrence (always positive).
    pub amount: f64,
    pub frequency: Frequency,
    /// First occurrence / due date. Later occurrences are derived from it.
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    /// Yearly change in the amount, in percent (raises, inflation, rate hikes).
    pub annual_change_pct: f64,
    pub autopay: bool,
    pub notes: String,
}

impl CashFlow {
    /// Average amount per month at today's prices.
    pub fn monthly_equivalent(&self) -> f64 {
        self.amount * self.frequency.per_year() / 12.0
    }

    /// The n-th scheduled date (n = 0 is `start_date`).
    fn nth_date(&self, n: u32) -> Option<NaiveDate> {
        let s = self.start_date;
        match self.frequency {
            Frequency::Once => (n == 0).then_some(s),
            Frequency::Weekly => s.checked_add_signed(Duration::days(7 * i64::from(n))),
            Frequency::Biweekly => s.checked_add_signed(Duration::days(14 * i64::from(n))),
            Frequency::SemiMonthly => {
                // Two payments per month: on the start day and ~15 days later.
                let base = add_months_clamped(s, n / 2, s.day())?;
                if n.is_multiple_of(2) {
                    Some(base)
                } else {
                    let second_day = s.day() + 15;
                    let first_of_month = base.with_day(1)?;
                    let last = last_day_of_month(first_of_month);
                    if second_day > last {
                        // Wrap into next month, e.g. 20th + 15 -> 5th of next month.
                        let next = add_months_clamped(first_of_month, 1, 1)?;
                        next.with_day(second_day - last)
                    } else {
                        base.with_day(second_day)
                    }
                }
            }
            Frequency::Monthly => add_months_clamped(s, n, s.day()),
            Frequency::Quarterly => add_months_clamped(s, n.checked_mul(3)?, s.day()),
            Frequency::SemiAnnually => add_months_clamped(s, n.checked_mul(6)?, s.day()),
            Frequency::Annually => add_months_clamped(s, n.checked_mul(12)?, s.day()),
        }
    }

    /// All occurrence dates within `[from, to]` inclusive.
    pub fn occurrences_between(&self, from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
        let mut out = Vec::new();
        if to < from {
            return out;
        }
        let end = match self.end_date {
            Some(e) if e < to => e,
            _ => to,
        };
        // Skip ahead roughly to `from` to avoid iterating decades of history.
        let mut n: u32 = if self.frequency.per_year() > 0.0 && from > self.start_date {
            let days = (from - self.start_date).num_days().max(0) as f64;
            let approx = (days / 365.25 * self.frequency.per_year()).floor() as i64 - 2;
            approx.max(0) as u32
        } else {
            0
        };
        while let Some(d) = self.nth_date(n) {
            if d > end {
                break;
            }
            if d >= from {
                out.push(d);
            }
            n += 1;
            if n > 1_000_000 {
                break;
            }
        }
        out
    }

    /// The next occurrence on or after `today`, if any.
    pub fn next_due(&self, today: NaiveDate) -> Option<NaiveDate> {
        self.occurrences_between(today, today + Duration::days(400 * 30))
            .into_iter()
            .next()
    }
}

pub fn last_day_of_month(d: NaiveDate) -> u32 {
    let first = d.with_day(1).expect("day 1 exists");
    let next = first.checked_add_months(Months::new(1)).expect("date in range");
    (next - Duration::days(1)).day()
}

/// Add `months` to `d`, then set the day to `day` clamped to the month's length
/// (so a bill due on the 31st falls on Feb 28/29).
pub fn add_months_clamped(d: NaiveDate, months: u32, day: u32) -> Option<NaiveDate> {
    let first = d.with_day(1)?.checked_add_months(Months::new(months))?;
    first.with_day(day.min(last_day_of_month(first)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountKind {
    Checking,
    Savings,
    Brokerage,
    Retirement401k,
    TraditionalIra,
    RothIra,
    Hsa,
    Pension,
    Other,
}

/// Which bucket an account is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountGroup {
    Cash,
    Investments,
    Retirement,
}

impl AccountKind {
    pub const ALL: [AccountKind; 9] = [
        AccountKind::Checking,
        AccountKind::Savings,
        AccountKind::Brokerage,
        AccountKind::Retirement401k,
        AccountKind::TraditionalIra,
        AccountKind::RothIra,
        AccountKind::Hsa,
        AccountKind::Pension,
        AccountKind::Other,
    ];

    pub fn key(self) -> &'static str {
        match self {
            AccountKind::Checking => "checking",
            AccountKind::Savings => "savings",
            AccountKind::Brokerage => "brokerage",
            AccountKind::Retirement401k => "401k",
            AccountKind::TraditionalIra => "traditional_ira",
            AccountKind::RothIra => "roth_ira",
            AccountKind::Hsa => "hsa",
            AccountKind::Pension => "pension",
            AccountKind::Other => "other",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.key() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            AccountKind::Checking => "Checking",
            AccountKind::Savings => "Savings",
            AccountKind::Brokerage => "Brokerage",
            AccountKind::Retirement401k => "401(k) / 403(b)",
            AccountKind::TraditionalIra => "Traditional IRA",
            AccountKind::RothIra => "Roth IRA",
            AccountKind::Hsa => "HSA",
            AccountKind::Pension => "Pension / TSP",
            AccountKind::Other => "Other asset",
        }
    }

    pub fn group(self) -> AccountGroup {
        match self {
            AccountKind::Checking | AccountKind::Savings => AccountGroup::Cash,
            AccountKind::Brokerage | AccountKind::Other => AccountGroup::Investments,
            AccountKind::Retirement401k
            | AccountKind::TraditionalIra
            | AccountKind::RothIra
            | AccountKind::Hsa
            | AccountKind::Pension => AccountGroup::Retirement,
        }
    }

    /// A sensible starting assumption for annual return, in percent.
    pub fn default_return_pct(self) -> f64 {
        match self {
            AccountKind::Checking => 0.0,
            AccountKind::Savings => 4.0,
            AccountKind::Other => 0.0,
            _ => 7.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub kind: AccountKind,
    /// Cash or manually entered value, not counting holdings.
    pub balance: f64,
    /// Expected yearly growth (APY for savings, average market return for investments).
    pub annual_return_pct: f64,
    /// Your own contribution per month.
    pub monthly_contribution: f64,
    /// True if the contribution is moved out of your checking account. False if it
    /// is withheld from your paycheck (income entered is already net of it).
    pub contribution_from_cash: bool,
    /// Employer match or other outside money added per month.
    pub employer_match_monthly: f64,
    pub notes: String,
    /// What the money is for (e.g. "Medical", "Food"). Empty = general.
    #[serde(default)]
    pub purpose: String,
    /// Operating baseline: the balance that should always stay in this account.
    /// Savings goals never count it as spendable.
    #[serde(default)]
    pub minimum_balance: f64,
}

/// Something to buy, funded from checking and savings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub id: i64,
    pub name: String,
    pub cost: f64,
    /// Optional "want it by" date.
    pub target_date: Option<NaiveDate>,
    /// Lower = earlier. Goals are planned in this order.
    pub priority: i64,
    pub notes: String,
    /// Set once bought; bought goals are no longer planned.
    pub purchased_on: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Holding {
    pub id: i64,
    pub account_id: i64,
    pub symbol: String,
    pub shares: f64,
    /// Last known price per share (entered manually in V1).
    pub price: f64,
    pub price_updated: Option<NaiveDate>,
}

impl Holding {
    pub fn value(&self) -> f64 {
        self.shares * self.price
    }
}

/// Interface size. `Auto` scales with the window; the others are fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UiScale {
    #[default]
    Auto,
    Small,
    Medium,
    Large,
    ExtraLarge,
}

impl UiScale {
    pub const ALL: [UiScale; 5] =
        [UiScale::Auto, UiScale::Small, UiScale::Medium, UiScale::Large, UiScale::ExtraLarge];
    /// Fixed sizes in ascending order (used by the +/- controls).
    const STEPS: [UiScale; 4] = [UiScale::Small, UiScale::Medium, UiScale::Large, UiScale::ExtraLarge];

    pub fn key(self) -> &'static str {
        match self {
            UiScale::Auto => "auto",
            UiScale::Small => "s",
            UiScale::Medium => "m",
            UiScale::Large => "l",
            UiScale::ExtraLarge => "xl",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.key() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            UiScale::Auto => "Auto",
            UiScale::Small => "S",
            UiScale::Medium => "M",
            UiScale::Large => "L",
            UiScale::ExtraLarge => "XL",
        }
    }

    pub fn long_label(self) -> &'static str {
        match self {
            UiScale::Auto => "Auto (fit window)",
            UiScale::Small => "Small",
            UiScale::Medium => "Medium",
            UiScale::Large => "Large",
            UiScale::ExtraLarge => "Extra large",
        }
    }

    /// CSS `font-size` for the root element. Every size in the UI is in rem, so this
    /// scales the whole interface. `Auto` grows with the window's width and height.
    pub fn root_font_size(self) -> &'static str {
        match self {
            UiScale::Auto => "clamp(13px, min(calc(8px + 0.55vw), calc(6px + 1.1vh)), 20px)",
            UiScale::Small => "14px",
            UiScale::Medium => "16px",
            UiScale::Large => "18px",
            UiScale::ExtraLarge => "20px",
        }
    }

    /// One step bigger. From `Auto`, goes to Large.
    pub fn bigger(self) -> Self {
        match self {
            UiScale::Auto => UiScale::Large,
            s => {
                let i = Self::STEPS.iter().position(|x| *x == s).unwrap_or(1);
                Self::STEPS[(i + 1).min(Self::STEPS.len() - 1)]
            }
        }
    }

    /// One step smaller. From `Auto`, goes to Small.
    pub fn smaller(self) -> Self {
        match self {
            UiScale::Auto => UiScale::Small,
            s => {
                let i = Self::STEPS.iter().position(|x| *x == s).unwrap_or(1);
                Self::STEPS[i.saturating_sub(1)]
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub projection_years: u32,
    pub inflation_pct: f64,
    pub currency_symbol: String,
    pub dark_mode: bool,
    pub ui_scale: UiScale,
    /// Reverse the scroll direction (content follows the fingers, like a phone).
    pub natural_scroll: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            projection_years: 30,
            inflation_pct: 3.0,
            currency_symbol: "$".into(),
            dark_mode: false,
            ui_scale: UiScale::Auto,
            natural_scroll: false,
        }
    }
}

/// Everything stored in the database, loaded into memory.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppData {
    pub flows: Vec<CashFlow>,
    pub accounts: Vec<Account>,
    pub holdings: Vec<Holding>,
    #[serde(default)]
    pub goals: Vec<Goal>,
    pub settings: Settings,
    /// FRED settings (includes the API key, so never written to backups).
    #[serde(skip)]
    pub fred: crate::fred::FredConfig,
    /// Cached FRED values (re-fetched on sync, so not part of backups).
    #[serde(skip)]
    pub market: crate::fred::MarketData,
}

impl AppData {
    pub fn holdings_for(&self, account_id: i64) -> impl Iterator<Item = &Holding> {
        self.holdings.iter().filter(move |h| h.account_id == account_id)
    }

    /// Balance plus the market value of the account's holdings.
    pub fn account_value(&self, a: &Account) -> f64 {
        a.balance + self.holdings_for(a.id).map(Holding::value).sum::<f64>()
    }

    pub fn group_total(&self, g: AccountGroup) -> f64 {
        self.accounts
            .iter()
            .filter(|a| a.kind.group() == g)
            .map(|a| self.account_value(a))
            .sum()
    }

    pub fn net_worth(&self) -> f64 {
        self.accounts.iter().map(|a| self.account_value(a)).sum()
    }

    pub fn monthly_total(&self, pred: impl Fn(FlowKind) -> bool, today: NaiveDate) -> f64 {
        self.flows
            .iter()
            .filter(|f| pred(f.kind))
            .filter(|f| f.end_date.is_none_or(|e| e >= today))
            .map(CashFlow::monthly_equivalent)
            .sum()
    }

    /// Sum of the minimum balances of checking and savings accounts: cash that savings
    /// goals must leave untouched.
    pub fn cash_baseline(&self) -> f64 {
        self.accounts
            .iter()
            .filter(|a| a.kind.group() == AccountGroup::Cash)
            .map(|a| a.minimum_balance.max(0.0))
            .sum()
    }

    /// Your own monthly contributions that leave the checking account.
    pub fn monthly_cash_contributions(&self) -> f64 {
        self.accounts
            .iter()
            .filter(|a| a.contribution_from_cash)
            .map(|a| a.monthly_contribution)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn flow(freq: Frequency, start: NaiveDate) -> CashFlow {
        CashFlow {
            id: 1,
            kind: FlowKind::Bill,
            name: "x".into(),
            category: "c".into(),
            amount: 100.0,
            frequency: freq,
            start_date: start,
            end_date: None,
            annual_change_pct: 0.0,
            autopay: false,
            notes: String::new(),
        }
    }

    #[test]
    fn monthly_clamps_to_month_end() {
        let f = flow(Frequency::Monthly, d(2026, 1, 31));
        let occ = f.occurrences_between(d(2026, 1, 1), d(2026, 4, 30));
        assert_eq!(occ, vec![d(2026, 1, 31), d(2026, 2, 28), d(2026, 3, 31), d(2026, 4, 30)]);
    }

    #[test]
    fn biweekly_can_hit_three_times_a_month() {
        let f = flow(Frequency::Biweekly, d(2026, 1, 2));
        assert_eq!(f.occurrences_between(d(2026, 1, 1), d(2026, 1, 31)).len(), 3);
        assert_eq!(f.occurrences_between(d(2026, 2, 1), d(2026, 2, 28)).len(), 2);
    }

    #[test]
    fn semimonthly_two_per_month() {
        let f = flow(Frequency::SemiMonthly, d(2026, 1, 1));
        let occ = f.occurrences_between(d(2026, 1, 1), d(2026, 2, 28));
        assert_eq!(occ, vec![d(2026, 1, 1), d(2026, 1, 16), d(2026, 2, 1), d(2026, 2, 16)]);
        let late = flow(Frequency::SemiMonthly, d(2026, 1, 20));
        let occ = late.occurrences_between(d(2026, 1, 1), d(2026, 2, 28));
        assert_eq!(occ, vec![d(2026, 1, 20), d(2026, 2, 4), d(2026, 2, 20)]);
    }

    #[test]
    fn end_date_and_once() {
        let mut f = flow(Frequency::Monthly, d(2026, 1, 5));
        f.end_date = Some(d(2026, 3, 1));
        assert_eq!(f.occurrences_between(d(2025, 1, 1), d(2030, 1, 1)).len(), 2);
        let o = flow(Frequency::Once, d(2027, 6, 1));
        assert_eq!(o.occurrences_between(d(2026, 1, 1), d(2030, 1, 1)), vec![d(2027, 6, 1)]);
        assert_eq!(o.monthly_equivalent(), 0.0);
    }

    #[test]
    fn skip_ahead_does_not_miss_dates() {
        let f = flow(Frequency::Weekly, d(2000, 1, 3));
        let occ = f.occurrences_between(d(2026, 3, 1), d(2026, 3, 31));
        assert!(occ.len() == 4 || occ.len() == 5);
        assert!(occ.iter().all(|x| (*x - d(2000, 1, 3)).num_days() % 7 == 0));
        assert_eq!(f.next_due(d(2026, 3, 1)), occ.first().copied());
    }

    #[test]
    fn ui_scale_steps() {
        assert_eq!(UiScale::Auto.bigger(), UiScale::Large);
        assert_eq!(UiScale::Auto.smaller(), UiScale::Small);
        assert_eq!(UiScale::ExtraLarge.bigger(), UiScale::ExtraLarge);
        assert_eq!(UiScale::Small.smaller(), UiScale::Small);
        assert_eq!(UiScale::Medium.bigger(), UiScale::Large);
        for s in UiScale::ALL {
            assert_eq!(UiScale::from_key(s.key()), Some(s));
        }
    }

    #[test]
    fn old_backups_without_new_settings_still_load() {
        let s: Settings = serde_json::from_str(r#"{"projection_years":20,"inflation_pct":2.5,"currency_symbol":"$","dark_mode":true}"#).unwrap();
        assert_eq!(s.projection_years, 20);
        assert_eq!(s.ui_scale, UiScale::Auto);
        assert!(!s.natural_scroll);
    }

    #[test]
    fn monthly_equivalent() {
        let f = flow(Frequency::Biweekly, d(2026, 1, 1));
        assert!((f.monthly_equivalent() - 216.666).abs() < 0.01);
    }
}
