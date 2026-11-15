//! Month-by-month projection of cash, investments and retirement balances.

use crate::model::{AccountGroup, AccountKind, AppData, FlowKind, add_months_clamped};
use chrono::{Datelike, Duration, NaiveDate};

#[derive(Debug, Clone, PartialEq)]
pub struct MonthPoint {
    /// First day of the month.
    pub month: NaiveDate,
    pub income: f64,
    pub expenses: f64,
    pub bills: f64,
    /// Own contributions taken from cash this month.
    pub cash_contributions: f64,
    pub cash: f64,
    pub investments: f64,
    pub retirement: f64,
    /// Inflation discount factor for this month (divide by it to get today's dollars).
    pub inflation_factor: f64,
}

impl MonthPoint {
    pub fn net_cashflow(&self) -> f64 {
        self.income - self.expenses - self.bills - self.cash_contributions
    }
    pub fn net_worth(&self) -> f64 {
        self.cash + self.investments + self.retirement
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct YearSummary {
    pub year: i32,
    pub income: f64,
    pub outflows: f64,
    pub contributions: f64,
    pub cash: f64,
    pub investments: f64,
    pub retirement: f64,
    pub inflation_factor: f64,
}

impl YearSummary {
    pub fn net_worth(&self) -> f64 {
        self.cash + self.investments + self.retirement
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Projection {
    pub months: Vec<MonthPoint>,
    /// First month where total cash goes below zero.
    pub cash_shortfall: Option<NaiveDate>,
}

impl Projection {
    pub fn yearly(&self) -> Vec<YearSummary> {
        let mut out: Vec<YearSummary> = Vec::new();
        for m in &self.months {
            let y = m.month.year();
            if out.last().is_none_or(|s| s.year != y) {
                out.push(YearSummary {
                    year: y,
                    income: 0.0,
                    outflows: 0.0,
                    contributions: 0.0,
                    cash: 0.0,
                    investments: 0.0,
                    retirement: 0.0,
                    inflation_factor: 1.0,
                });
            }
            let s = out.last_mut().expect("just pushed");
            s.income += m.income;
            s.outflows += m.expenses + m.bills;
            s.contributions += m.cash_contributions;
            // End-of-year balances: keep overwriting with the latest month.
            s.cash = m.cash;
            s.investments = m.investments;
            s.retirement = m.retirement;
            s.inflation_factor = m.inflation_factor;
        }
        out
    }

    pub fn at_years(&self, years: u32) -> Option<&MonthPoint> {
        let idx = (years as usize * 12).saturating_sub(1);
        self.months.get(idx).or(self.months.last())
    }
}

fn monthly_rate(annual_pct: f64) -> f64 {
    (1.0 + annual_pct / 100.0).powf(1.0 / 12.0) - 1.0
}

/// Simulate `years` years starting with the month containing `today`.
///
/// Rules:
/// * Each month, every account grows by its own expected return, then receives its
///   contributions (yours + employer match).
/// * Income minus expenses, bills and contributions "from cash" lands in the primary
///   cash account (first checking account, else first savings, else a virtual one).
/// * Cash-flow amounts change by their own `annual_change_pct` once per year elapsed.
/// * In the current month only occurrences from `today` onward are counted.
pub fn project(data: &AppData, today: NaiveDate, years: u32) -> Projection {
    let months_total = years.max(1) * 12;
    let start_month = today.with_day(1).expect("day 1");
    let infl_m = monthly_rate(data.settings.inflation_pct);

    // Per-account simulated balances.
    let mut balances: Vec<f64> = data.accounts.iter().map(|a| data.account_value(a)).collect();
    let primary = data
        .accounts
        .iter()
        .position(|a| a.kind == AccountKind::Checking)
        .or_else(|| data.accounts.iter().position(|a| a.kind.group() == AccountGroup::Cash));
    let mut virtual_cash = 0.0;

    let mut proj = Projection::default();
    let mut infl_factor = 1.0;

    for m in 0..months_total {
        let month = add_months_clamped(start_month, m, 1).expect("date in range");
        let next_month = add_months_clamped(start_month, m + 1, 1).expect("date in range");
        let from = if m == 0 { today } else { month };
        let to = next_month - Duration::days(1);
        let years_elapsed = (m / 12) as i32;

        let (mut income, mut expenses, mut bills) = (0.0, 0.0, 0.0);
        for f in &data.flows {
            let n = f.occurrences_between(from, to).len();
            if n == 0 {
                continue;
            }
            let amt = f.amount * (1.0 + f.annual_change_pct / 100.0).powi(years_elapsed) * n as f64;
            match f.kind {
                FlowKind::Income => income += amt,
                FlowKind::Expense => expenses += amt,
                FlowKind::Bill => bills += amt,
            }
        }

        // Growth and contributions.
        let mut cash_contrib = 0.0;
        for (i, a) in data.accounts.iter().enumerate() {
            balances[i] *= 1.0 + monthly_rate(a.annual_return_pct);
            balances[i] += a.monthly_contribution + a.employer_match_monthly;
            if a.contribution_from_cash {
                cash_contrib += a.monthly_contribution;
            }
        }

        let net = income - expenses - bills - cash_contrib;
        match primary {
            Some(p) => balances[p] += net,
            None => virtual_cash += net,
        }

        infl_factor *= 1.0 + infl_m;
        let mut pt = MonthPoint {
            month,
            income,
            expenses,
            bills,
            cash_contributions: cash_contrib,
            cash: virtual_cash,
            investments: 0.0,
            retirement: 0.0,
            inflation_factor: infl_factor,
        };
        for (i, a) in data.accounts.iter().enumerate() {
            match a.kind.group() {
                AccountGroup::Cash => pt.cash += balances[i],
                AccountGroup::Investments => pt.investments += balances[i],
                AccountGroup::Retirement => pt.retirement += balances[i],
            }
        }
        if proj.cash_shortfall.is_none() && pt.cash < 0.0 {
            proj.cash_shortfall = Some(month);
        }
        proj.months.push(pt);
    }
    proj
}

/// An upcoming payment, for the "next 30 days" list.
#[derive(Debug, Clone, PartialEq)]
pub struct Upcoming {
    pub date: NaiveDate,
    pub flow_id: i64,
    pub name: String,
    pub kind: FlowKind,
    pub amount: f64,
    pub autopay: bool,
}

pub fn upcoming(data: &AppData, today: NaiveDate, days: i64) -> Vec<Upcoming> {
    let to = today + Duration::days(days);
    let mut out: Vec<Upcoming> = data
        .flows
        .iter()
        .flat_map(|f| {
            f.occurrences_between(today, to).into_iter().map(move |date| Upcoming {
                date,
                flow_id: f.id,
                name: f.name.clone(),
                kind: f.kind,
                amount: f.amount,
                autopay: f.autopay,
            })
        })
        .collect();
    out.sort_by(|a, b| a.date.cmp(&b.date).then(a.name.cmp(&b.name)));
    out
}

/// Monthly outflow by category, largest first.
pub fn spending_by_category(data: &AppData, today: NaiveDate) -> Vec<(String, f64)> {
    let mut map: std::collections::BTreeMap<String, f64> = Default::default();
    for f in data.flows.iter().filter(|f| f.kind.is_outflow()) {
        if f.end_date.is_some_and(|e| e < today) {
            continue;
        }
        let cat = if f.category.trim().is_empty() { "Uncategorized".to_string() } else { f.category.clone() };
        *map.entry(cat).or_default() += f.monthly_equivalent();
    }
    let mut v: Vec<_> = map.into_iter().filter(|(_, v)| *v > 0.0).collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn account(id: i64, kind: AccountKind, bal: f64, ret: f64, contrib: f64, from_cash: bool) -> Account {
        Account {
            id,
            name: format!("a{id}"),
            kind,
            balance: bal,
            annual_return_pct: ret,
            monthly_contribution: contrib,
            contribution_from_cash: from_cash,
            employer_match_monthly: 0.0,
            notes: String::new(),
            purpose: String::new(),
            minimum_balance: 0.0,
        }
    }

    fn cf(kind: FlowKind, amount: f64, freq: Frequency, start: NaiveDate) -> CashFlow {
        CashFlow {
            id: 0,
            kind,
            name: "f".into(),
            category: "Cat".into(),
            amount,
            frequency: freq,
            start_date: start,
            end_date: None,
            annual_change_pct: 0.0,
            autopay: false,
            notes: String::new(),
        }
    }

    #[test]
    fn simple_cash_accumulation() {
        let mut data = AppData::default();
        data.settings.inflation_pct = 0.0;
        data.accounts.push(account(1, AccountKind::Checking, 1000.0, 0.0, 0.0, false));
        data.flows.push(cf(FlowKind::Income, 5000.0, Frequency::Monthly, d(2026, 1, 1)));
        data.flows.push(cf(FlowKind::Bill, 2000.0, Frequency::Monthly, d(2026, 1, 5)));
        data.flows.push(cf(FlowKind::Expense, 500.0, Frequency::Monthly, d(2026, 1, 10)));
        let p = project(&data, d(2026, 1, 1), 1);
        assert_eq!(p.months.len(), 12);
        assert!((p.months[11].cash - (1000.0 + 12.0 * 2500.0)).abs() < 1e-6);
        assert!(p.cash_shortfall.is_none());
        assert_eq!(p.yearly().len(), 1);
        assert!((p.yearly()[0].income - 60000.0).abs() < 1e-6);
    }

    #[test]
    fn current_month_skips_past_occurrences() {
        let mut data = AppData::default();
        data.accounts.push(account(1, AccountKind::Checking, 0.0, 0.0, 0.0, false));
        data.flows.push(cf(FlowKind::Income, 100.0, Frequency::Monthly, d(2026, 1, 1)));
        let p = project(&data, d(2026, 3, 15), 1);
        assert_eq!(p.months[0].income, 0.0);
        assert_eq!(p.months[1].income, 100.0);
    }

    #[test]
    fn investment_growth_and_contributions() {
        let mut data = AppData::default();
        data.accounts.push(account(1, AccountKind::Checking, 10_000.0, 0.0, 0.0, false));
        data.accounts.push(account(2, AccountKind::Retirement401k, 100_000.0, 7.0, 0.0, false));
        data.accounts.push(account(3, AccountKind::Brokerage, 0.0, 0.0, 500.0, true));
        let p = project(&data, d(2026, 1, 1), 10);
        let last = p.months.last().unwrap();
        // 100k at 7% for 10 years.
        assert!((last.retirement - 100_000.0 * 1.07f64.powi(10)).abs() < 1.0);
        // Brokerage funded from checking: 120 months * 500.
        assert!((last.investments - 60_000.0).abs() < 1e-6);
        assert!((last.cash - (10_000.0 - 60_000.0)).abs() < 1e-6);
        assert_eq!(p.cash_shortfall, Some(d(2027, 9, 1)));
    }

    #[test]
    fn holdings_count_toward_account_value() {
        let mut data = AppData::default();
        data.accounts.push(account(1, AccountKind::Brokerage, 50.0, 0.0, 0.0, false));
        data.holdings.push(Holding { id: 1, account_id: 1, symbol: "VTI".into(), shares: 10.0, price: 250.0, price_updated: None });
        assert_eq!(data.net_worth(), 2550.0);
        let p = project(&data, d(2026, 1, 1), 1);
        assert_eq!(p.months[0].investments, 2550.0);
    }

    #[test]
    fn annual_raise_applies_each_year() {
        let mut data = AppData::default();
        let mut inc = cf(FlowKind::Income, 1000.0, Frequency::Monthly, d(2026, 1, 1));
        inc.annual_change_pct = 10.0;
        data.flows.push(inc);
        let p = project(&data, d(2026, 1, 1), 3);
        assert!((p.months[0].income - 1000.0).abs() < 1e-9);
        assert!((p.months[12].income - 1100.0).abs() < 1e-9);
        assert!((p.months[24].income - 1210.0).abs() < 1e-9);
        // No accounts: cash goes to a virtual bucket.
        assert!(p.months[0].cash > 0.0);
    }

    #[test]
    fn upcoming_and_categories() {
        let mut data = AppData::default();
        let mut a = cf(FlowKind::Bill, 50.0, Frequency::Monthly, d(2026, 1, 20));
        a.category = "Utilities".into();
        let mut b = cf(FlowKind::Expense, 300.0, Frequency::Weekly, d(2026, 1, 1));
        b.category = "Food".into();
        data.flows.push(a);
        data.flows.push(b);
        data.flows.push(cf(FlowKind::Income, 9000.0, Frequency::Monthly, d(2026, 1, 1)));
        let up = upcoming(&data, d(2026, 2, 1), 30);
        assert!(up.windows(2).all(|w| w[0].date <= w[1].date));
        assert_eq!(up.iter().filter(|u| u.kind == FlowKind::Bill).count(), 1);
        let cats = spending_by_category(&data, d(2026, 2, 1));
        assert_eq!(cats[0].0, "Food");
        assert_eq!(cats.len(), 2);
    }
}
