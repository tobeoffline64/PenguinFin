//! Debts and payoff plans (avalanche / snowball).
//!
//! Interest accrues monthly at APR / 12 on the current balance. Each month every debt gets
//! its minimum payment (or the remaining balance, if smaller). The plan's monthly budget is
//! the sum of all original minimums plus an extra amount; whatever is left after the
//! minimums goes to the target debt (highest APR first for avalanche, smallest balance first
//! for snowball). When a debt is paid off its minimum "rolls over" to the next target, so
//! the total monthly payment stays the same until everything is paid.

use crate::model::add_months_clamped;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebtKind {
    CreditCard,
    PersonalLoan,
    StudentLoan,
    AutoLoan,
    Medical,
    Other,
}

impl DebtKind {
    pub const ALL: [DebtKind; 6] = [
        DebtKind::CreditCard,
        DebtKind::PersonalLoan,
        DebtKind::StudentLoan,
        DebtKind::AutoLoan,
        DebtKind::Medical,
        DebtKind::Other,
    ];
    pub fn key(self) -> &'static str {
        match self {
            DebtKind::CreditCard => "credit_card",
            DebtKind::PersonalLoan => "personal_loan",
            DebtKind::StudentLoan => "student_loan",
            DebtKind::AutoLoan => "auto_loan",
            DebtKind::Medical => "medical",
            DebtKind::Other => "other",
        }
    }
    pub fn from_key(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.key() == s)
    }
    pub fn label(self) -> &'static str {
        match self {
            DebtKind::CreditCard => "Credit card",
            DebtKind::PersonalLoan => "Personal loan",
            DebtKind::StudentLoan => "Student loan",
            DebtKind::AutoLoan => "Auto loan",
            DebtKind::Medical => "Medical bill",
            DebtKind::Other => "Other debt",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Debt {
    pub id: i64,
    pub name: String,
    pub kind: DebtKind,
    /// Amount owed today.
    pub balance: f64,
    /// Annual percentage rate, e.g. 24.99.
    pub apr: f64,
    /// Required monthly payment.
    pub min_payment: f64,
    /// Account the payments come from (`None` = first checking account).
    pub account_id: Option<i64>,
    pub notes: String,
}

impl Debt {
    pub fn monthly_rate(&self) -> f64 {
        self.apr.max(0.0) / 100.0 / 12.0
    }
    /// Interest charged on today's balance over one month.
    pub fn monthly_interest(&self) -> f64 {
        self.balance.max(0.0) * self.monthly_rate()
    }
}

/// A one-time payment toward a debt: either already made ("logged", which reduced the debt
/// balance when it was recorded) or planned for a future date (applied by the plan and
/// projections in that month).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebtPayment {
    pub id: i64,
    pub debt_id: i64,
    pub date: NaiveDate,
    pub amount: f64,
    /// Account the money comes from (`None` = the debt's own paying account).
    pub account_id: Option<i64>,
    pub notes: String,
    /// True once made and recorded; false while it's planned.
    pub logged: bool,
    /// For logged payments: how much the debt balance actually went down (capped at what
    /// was owed), so undoing restores it exactly.
    pub applied: f64,
    /// For logged payments: whether the amount was also taken off the account's balance.
    pub deducted: bool,
}

/// A planned one-time payment for the simulator: (debt index, amount).
pub type Lump = (usize, f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Strategy {
    /// Highest interest rate first: least interest paid.
    #[default]
    Avalanche,
    /// Smallest balance first: quickest wins.
    Snowball,
}

impl Strategy {
    pub fn key(self) -> &'static str {
        match self {
            Strategy::Avalanche => "avalanche",
            Strategy::Snowball => "snowball",
        }
    }
    pub fn from_key(s: &str) -> Option<Self> {
        match s {
            "avalanche" => Some(Strategy::Avalanche),
            "snowball" => Some(Strategy::Snowball),
            _ => None,
        }
    }
}

/// Monthly payment that pays off `balance` in `months` at `apr` (standard amortization).
pub fn payment_for_months(balance: f64, apr: f64, months: u32) -> f64 {
    if balance <= 0.0 || months == 0 {
        return balance.max(0.0);
    }
    let r = apr.max(0.0) / 100.0 / 12.0;
    if r == 0.0 {
        return balance / months as f64;
    }
    balance * r / (1.0 - (1.0 + r).powi(-(months as i32)))
}

/// Steps a set of debts forward one month at a time. Shared by the payoff plan and the
/// main projection so both always agree.
#[derive(Debug, Clone)]
pub struct Simulator {
    rates: Vec<f64>,
    mins: Vec<f64>,
    balances: Vec<f64>,
    /// Debt indices in payoff order (the target is the first one still owed).
    order: Vec<usize>,
    /// Total paid per month while any debt remains (minimums + extra), when rolling over.
    budget: f64,
    rollover: bool,
}

/// What happened to each debt in one month.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MonthStep {
    pub interest: Vec<f64>,
    /// Everything paid toward each debt this month (regular + one-time).
    pub payments: Vec<f64>,
    /// The one-time part of `payments`.
    pub one_time: Vec<f64>,
}

impl MonthStep {
    pub fn total_payment(&self) -> f64 {
        self.payments.iter().sum()
    }
    pub fn total_interest(&self) -> f64 {
        self.interest.iter().sum()
    }
}

impl Simulator {
    /// `extra` is paid on top of the minimums each month. With `rollover = false` (used for
    /// "minimums only") each debt only ever gets its own minimum.
    pub fn new(debts: &[Debt], strategy: Strategy, extra: f64, rollover: bool) -> Self {
        let mut order: Vec<usize> = (0..debts.len()).collect();
        match strategy {
            Strategy::Avalanche => order.sort_by(|&a, &b| {
                debts[b].apr.total_cmp(&debts[a].apr).then(debts[a].balance.total_cmp(&debts[b].balance))
            }),
            Strategy::Snowball => order.sort_by(|&a, &b| {
                debts[a].balance.total_cmp(&debts[b].balance).then(debts[b].apr.total_cmp(&debts[a].apr))
            }),
        }
        let mins: Vec<f64> = debts.iter().map(|d| d.min_payment.max(0.0)).collect();
        Simulator {
            rates: debts.iter().map(Debt::monthly_rate).collect(),
            budget: mins.iter().sum::<f64>() + extra.max(0.0),
            mins,
            balances: debts.iter().map(|d| d.balance.max(0.0)).collect(),
            order,
            rollover,
        }
    }

    pub fn balances(&self) -> &[f64] {
        &self.balances
    }

    pub fn total_balance(&self) -> f64 {
        self.balances.iter().sum()
    }

    pub fn is_paid_off(&self) -> bool {
        self.total_balance() < 0.005
    }

    /// Payoff order (debt indices).
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    pub fn step(&mut self) -> MonthStep {
        self.step_with(&[])
    }

    /// One month, plus one-time payments. Each one-time payment goes to its own debt and is
    /// capped at what's still owed (it never overpays or spills onto other debts).
    pub fn step_with(&mut self, lumps: &[Lump]) -> MonthStep {
        let n = self.balances.len();
        let mut s = MonthStep { interest: vec![0.0; n], payments: vec![0.0; n], one_time: vec![0.0; n] };
        if self.is_paid_off() {
            return s;
        }
        for i in 0..n {
            if self.balances[i] > 0.0 {
                let int = self.balances[i] * self.rates[i];
                self.balances[i] += int;
                s.interest[i] = int;
            }
        }
        // Minimums first.
        for i in 0..n {
            let pay = self.mins[i].min(self.balances[i]);
            self.balances[i] -= pay;
            s.payments[i] = pay;
        }
        // Then the rest of the budget goes to targets in order.
        let mut left = if self.rollover {
            self.budget - s.total_payment()
        } else {
            self.budget - self.mins.iter().sum::<f64>()
        };
        for &i in &self.order {
            if left <= 0.0 {
                break;
            }
            let pay = left.min(self.balances[i]);
            self.balances[i] -= pay;
            s.payments[i] += pay;
            left -= pay;
        }
        for &(i, amount) in lumps {
            if i < n {
                let pay = amount.max(0.0).min(self.balances[i]);
                self.balances[i] -= pay;
                s.payments[i] += pay;
                s.one_time[i] += pay;
            }
        }
        for b in &mut self.balances {
            if *b < 0.005 {
                *b = 0.0;
            }
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanMonth {
    pub month: NaiveDate,
    pub payments: Vec<f64>,
    pub interest: Vec<f64>,
    /// Balances after this month's payments.
    pub balances: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PayoffPlan {
    pub months: Vec<PlanMonth>,
    /// Month each debt is paid off (`None` = not within the limit).
    pub payoff: Vec<Option<NaiveDate>>,
    /// Payoff order (debt indices).
    pub order: Vec<usize>,
    pub total_interest: f64,
    pub total_paid: f64,
    /// Month the last debt is paid off.
    pub debt_free: Option<NaiveDate>,
    /// Interest paid per debt over the plan.
    pub interest_by_debt: Vec<f64>,
}

impl PayoffPlan {
    pub fn months_to_free(&self) -> Option<usize> {
        self.debt_free.map(|_| self.months.len())
    }
}

/// Simulate until debt-free or `max_months`, starting in the month of `start`.
pub fn plan(debts: &[Debt], strategy: Strategy, extra: f64, rollover: bool, start: NaiveDate, max_months: u32) -> PayoffPlan {
    plan_with(debts, strategy, extra, rollover, start, max_months, &[])
}

/// One-time payments planned for each month, as simulator lumps: planned (not yet logged)
/// payments dated in or after `start`'s month. Index 0 = `start`'s month.
pub fn planned_lumps(debts: &[Debt], payments: &[DebtPayment], start: NaiveDate) -> Vec<(u32, Lump)> {
    let first = add_months_clamped(start, 0, 1).unwrap_or(start);
    payments
        .iter()
        .filter(|p| !p.logged && p.amount > 0.0)
        .filter_map(|p| {
            let i = debts.iter().position(|d| d.id == p.debt_id)?;
            let months = (p.date.year() - first.year()) * 12 + p.date.month() as i32 - first.month() as i32;
            (months >= 0).then_some((months as u32, (i, p.amount)))
        })
        .collect()
}

/// Like [`plan`], with one-time payments: `(month index, (debt index, amount))`.
pub fn plan_with(
    debts: &[Debt],
    strategy: Strategy,
    extra: f64,
    rollover: bool,
    start: NaiveDate,
    max_months: u32,
    lumps: &[(u32, Lump)],
) -> PayoffPlan {
    let mut sim = Simulator::new(debts, strategy, extra, rollover);
    let first = add_months_clamped(start, 0, 1).unwrap_or(start);
    let mut p = PayoffPlan {
        payoff: vec![None; debts.len()],
        order: sim.order().to_vec(),
        interest_by_debt: vec![0.0; debts.len()],
        ..Default::default()
    };
    for (i, d) in debts.iter().enumerate() {
        if d.balance <= 0.0 {
            p.payoff[i] = Some(first);
        }
    }
    for m in 0..max_months {
        if sim.is_paid_off() {
            break;
        }
        let month = add_months_clamped(first, m, 1).unwrap_or(first);
        let this_month: Vec<Lump> = lumps.iter().filter(|(k, _)| *k == m).map(|(_, l)| *l).collect();
        let s = sim.step_with(&this_month);
        p.total_interest += s.total_interest();
        p.total_paid += s.total_payment();
        for (i, x) in s.interest.iter().enumerate() {
            p.interest_by_debt[i] += x;
        }
        for (i, b) in sim.balances().iter().enumerate() {
            if *b == 0.0 && p.payoff[i].is_none() {
                p.payoff[i] = Some(month);
            }
        }
        p.months.push(PlanMonth { month, payments: s.payments, interest: s.interest, balances: sim.balances().to_vec() });
    }
    if sim.is_paid_off() {
        p.debt_free = p.months.last().map(|m| m.month).or(Some(first));
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, 1).unwrap()
    }

    fn debt(id: i64, balance: f64, apr: f64, min: f64) -> Debt {
        Debt { id, name: format!("d{id}"), kind: DebtKind::CreditCard, balance, apr, min_payment: min, account_id: None, notes: String::new() }
    }

    #[test]
    fn amortization_formula() {
        // $10,000 at 6% over 60 months → $193.33/mo (standard loan table value).
        assert!((payment_for_months(10_000.0, 6.0, 60) - 193.33).abs() < 0.01);
        assert!((payment_for_months(1200.0, 0.0, 12) - 100.0).abs() < 1e-9);
        // Paying that amount for that many months clears the balance.
        let p = payment_for_months(5000.0, 24.0, 24);
        let pl = plan(&[debt(1, 5000.0, 24.0, p)], Strategy::Avalanche, 0.0, true, d(2026, 1), 600);
        assert_eq!(pl.months.len(), 24);
        assert_eq!(pl.debt_free, Some(d(2027, 12)));
    }

    #[test]
    fn single_card_interest_and_payoff() {
        // $1,000 at 12% APR (1%/mo), $100/mo.
        let pl = plan(&[debt(1, 1000.0, 12.0, 100.0)], Strategy::Avalanche, 0.0, true, d(2026, 1), 600);
        assert!((pl.months[0].interest[0] - 10.0).abs() < 1e-9);
        assert!((pl.months[0].balances[0] - 910.0).abs() < 1e-9);
        assert_eq!(pl.months.len(), 11);
        // Total paid = principal + interest.
        assert!((pl.total_paid - (1000.0 + pl.total_interest)).abs() < 1e-6);
        assert!(pl.total_interest > 50.0 && pl.total_interest < 60.0, "{}", pl.total_interest);
    }

    #[test]
    fn avalanche_beats_snowball_on_interest_and_rollover_helps() {
        let debts = [debt(1, 2000.0, 25.0, 60.0), debt(2, 500.0, 5.0, 25.0), debt(3, 8000.0, 9.0, 160.0)];
        let start = d(2026, 1);
        let av = plan(&debts, Strategy::Avalanche, 300.0, true, start, 600);
        let sb = plan(&debts, Strategy::Snowball, 300.0, true, start, 600);
        let mins = plan(&debts, Strategy::Avalanche, 0.0, false, start, 600);
        assert_eq!(av.order, vec![0, 2, 1]); // 25%, 9%, 5%
        assert_eq!(sb.order, vec![1, 0, 2]); // $500, $2,000, $8,000
        assert!(av.total_interest <= sb.total_interest);
        assert!(av.months.len() < mins.months.len());
        assert!(av.total_interest < mins.total_interest);
        // Snowball pays the small one first.
        assert!(sb.payoff[1] < sb.payoff[0]);
        // Budget is constant while debts remain: 245 + 300.
        assert!((av.months[0].payments.iter().sum::<f64>() - 545.0).abs() < 1e-9);
        assert!((av.months[5].payments.iter().sum::<f64>() - 545.0).abs() < 1e-9);
    }

    #[test]
    fn one_time_payments_shorten_the_plan_and_are_capped() {
        let debts = [debt(1, 3000.0, 20.0, 100.0), debt(2, 1000.0, 10.0, 50.0)];
        let start = d(2026, 1);
        let base = plan(&debts, Strategy::Avalanche, 0.0, true, start, 600);
        // $1,000 toward the second debt in month 3 (April).
        let pays = [DebtPayment {
            id: 1,
            debt_id: 2,
            date: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
            amount: 1000.0,
            account_id: None,
            notes: "Tax refund".into(),
            logged: false,
            applied: 0.0,
            deducted: false,
        }];
        let lumps = planned_lumps(&debts, &pays, start);
        assert_eq!(lumps, vec![(3, (1, 1000.0))]);
        let with = plan_with(&debts, Strategy::Avalanche, 0.0, true, start, 600, &lumps);
        assert!(with.total_interest < base.total_interest);
        assert!(with.months.len() < base.months.len());
        // Debt 2 is paid off that month; the payment is capped at what was owed (< $1,000 + min).
        assert_eq!(with.payoff[1], Some(d(2026, 4)));
        assert!(with.months[3].payments[1] < 1000.0);
        // Logged payments (already applied to balances) and past ones are ignored.
        let logged = [DebtPayment { logged: true, ..pays[0].clone() }];
        assert!(planned_lumps(&debts, &logged, start).is_empty());
        assert!(planned_lumps(&debts, &pays, d(2026, 5)).is_empty());
    }

    #[test]
    fn minimum_below_interest_never_pays_off() {
        let pl = plan(&[debt(1, 10_000.0, 30.0, 100.0)], Strategy::Avalanche, 0.0, true, d(2026, 1), 120);
        assert_eq!(pl.debt_free, None);
        assert_eq!(pl.payoff[0], None);
        assert!(pl.months.last().unwrap().balances[0] > 10_000.0);
    }
}
