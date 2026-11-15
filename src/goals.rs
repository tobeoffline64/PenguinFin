//! Savings goals: when each planned purchase becomes affordable.
//!
//! Spendable cash = projected checking + savings balance − every cash account's minimum
//! balance. Goals are planned in priority order. A goal is affordable in month *m* only if,
//! after paying for it (and all goals before it), spendable cash stays ≥ 0 in month *m* and
//! in every later month of the projection; a purchase that would make a later bill dip into
//! the baselines is not counted as affordable yet. Interest that the spent money would have
//! earned is ignored (it's small next to the purchase itself).

use crate::model::{AccountGroup, AppData, last_day_of_month};
use crate::projection::project;
use chrono::{Datelike, NaiveDate};

#[derive(Debug, Clone, PartialEq)]
pub enum TargetStatus {
    /// Affordable on or before the "want it by" date.
    OnTrack,
    /// Not affordable by the date; saving this much more per month would close the gap.
    Behind { extra_per_month: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoalPlan {
    pub goal_id: i64,
    /// First month the goal is affordable (`today` if it already is). `None` = not within
    /// the planning horizon.
    pub affordable_on: Option<NaiveDate>,
    /// Whole months from now until `affordable_on`.
    pub months_away: Option<u32>,
    /// How much of the cost today's spendable cash already covers, after earlier goals (0–1).
    pub progress: f64,
    pub target: Option<TargetStatus>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GoalsPlan {
    /// Checking + savings balance today.
    pub cash_now: f64,
    /// Sum of minimum balances kept in checking and savings.
    pub baseline: f64,
    /// Money in checking/savings buckets today (also reserved, not spendable).
    pub buckets_now: f64,
    /// Spendable today: cash_now − baseline − buckets_now (may be negative).
    pub spendable_now: f64,
    /// Plans for open goals, in priority order.
    pub plans: Vec<GoalPlan>,
    /// Spendable cash per month (first entry = today) with planned purchases taken out.
    pub spendable_after_goals: Vec<(NaiveDate, f64)>,
    pub horizon_years: u32,
}

impl GoalsPlan {
    pub fn plan_for(&self, goal_id: i64) -> Option<&GoalPlan> {
        self.plans.iter().find(|p| p.goal_id == goal_id)
    }
}

fn months_between(from: NaiveDate, to: NaiveDate) -> u32 {
    let m = (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32;
    m.max(0) as u32
}

pub fn plan_goals(data: &AppData, today: NaiveDate, years: u32) -> GoalsPlan {
    let years = years.max(1);
    let baseline = data.cash_baseline();
    let cash_now = data.group_total(AccountGroup::Cash);
    let buckets_now = data.cash_buckets_total();
    let proj = project(data, today, years);
    // Positions (in the projection) of buckets that live in checking/savings accounts.
    let cash_bucket_pos: Vec<usize> = proj
        .bucket_ids
        .iter()
        .enumerate()
        .filter(|(_, id)| {
            data.bucket(**id).is_some_and(|b| {
                data.accounts.iter().any(|a| a.id == b.account_id && a.kind.group() == AccountGroup::Cash)
            })
        })
        .map(|(i, _)| i)
        .collect();

    // Index 0 = today; index k = the last day of projected month k-1 (its balance is in).
    let mut dates = vec![today];
    let mut spendable = vec![cash_now - baseline - buckets_now];
    for m in &proj.months {
        dates.push(m.month.with_day(last_day_of_month(m.month)).unwrap_or(m.month));
        let reserved_in_buckets: f64 = cash_bucket_pos.iter().map(|&i| m.buckets[i]).sum();
        spendable.push(m.cash - baseline - reserved_in_buckets);
    }
    let n = spendable.len();
    // suffix_min[k] = lowest spendable cash from month k to the end of the horizon.
    let mut suffix_min = vec![f64::INFINITY; n + 1];
    for k in (0..n).rev() {
        suffix_min[k] = suffix_min[k + 1].min(spendable[k]);
    }

    let mut open: Vec<_> = data.goals.iter().filter(|g| g.purchased_on.is_none()).collect();
    open.sort_by_key(|g| (g.priority, g.id));

    let mut committed = 0.0; // cost of goals planned so far
    let mut purchases: Vec<(usize, f64)> = Vec::new();
    let mut plans = Vec::new();
    for g in open {
        let cost = g.cost.max(0.0);
        let idx = (0..n).find(|&k| suffix_min[k] - committed >= cost);
        let progress = if cost <= 0.0 { 1.0 } else { ((spendable[0] - committed) / cost).clamp(0.0, 1.0) };
        let affordable_on = idx.map(|k| dates[k]);
        let target = g.target_date.map(|t| match affordable_on {
            Some(d) if d <= t => TargetStatus::OnTrack,
            _ => {
                // Spendable cash that's safely available by the target month, after earlier goals.
                let k_target = dates.iter().rposition(|d| *d <= t).unwrap_or(0);
                let have = (suffix_min[k_target] - committed).max(0.0);
                let months = months_between(today, t).max(1) as f64;
                TargetStatus::Behind { extra_per_month: ((cost - have).max(0.0) / months).max(0.01) }
            }
        });
        if let Some(k) = idx {
            committed += cost;
            purchases.push((k, cost));
        }
        plans.push(GoalPlan {
            goal_id: g.id,
            affordable_on,
            months_away: affordable_on.map(|d| months_between(today, d)),
            progress,
            target,
        });
    }

    let spendable_after_goals = (0..n)
        .map(|k| {
            let spent: f64 = purchases.iter().filter(|(pk, _)| *pk <= k).map(|(_, c)| c).sum();
            (dates[k], spendable[k] - spent)
        })
        .collect();

    GoalsPlan {
        cash_now,
        baseline,
        buckets_now,
        spendable_now: cash_now - baseline - buckets_now,
        plans,
        spendable_after_goals,
        horizon_years: years,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn checking(balance: f64, minimum: f64) -> Account {
        Account {
            id: 1,
            name: "Checking".into(),
            kind: AccountKind::Checking,
            balance,
            annual_return_pct: 0.0,
            monthly_contribution: 0.0,
            contribution_from_cash: false,
            employer_match_monthly: 0.0,
            notes: String::new(),
            purpose: String::new(),
            minimum_balance: minimum,
        }
    }

    fn goal(id: i64, cost: f64, priority: i64, target: Option<NaiveDate>) -> Goal {
        Goal { id, name: format!("g{id}"), cost, target_date: target, priority, notes: String::new(), purchased_on: None }
    }

    fn income(amount: f64) -> CashFlow {
        CashFlow {
            id: 1,
            kind: FlowKind::Income,
            name: "pay".into(),
            category: String::new(),
            amount,
            frequency: Frequency::Monthly,
            start_date: d(2026, 1, 1),
            end_date: None,
            annual_change_pct: 0.0,
            autopay: false,
            notes: String::new(),
            account_id: None,
            bucket_id: None,
        }
    }

    /// Saves $1,000/month, starts with $5,000, keeps $2,000 → $3,000 spendable now.
    fn base() -> AppData {
        let mut data = AppData::default();
        data.settings.inflation_pct = 0.0;
        data.accounts.push(checking(5000.0, 2000.0));
        data.flows.push(income(1000.0));
        data
    }

    #[test]
    fn affordable_now_and_later_in_priority_order() {
        let mut data = base();
        data.goals.push(goal(1, 2500.0, 1, None)); // fits today's $3,000
        data.goals.push(goal(2, 2000.0, 2, None)); // $500 left today; +$1,000/mo → 2 months
        let p = plan_goals(&data, d(2026, 1, 1), 2);
        assert_eq!(p.baseline, 2000.0);
        assert_eq!(p.spendable_now, 3000.0);
        assert_eq!(p.plan_for(1).unwrap().affordable_on, Some(d(2026, 1, 1)));
        assert_eq!(p.plan_for(1).unwrap().progress, 1.0);
        let g2 = p.plan_for(2).unwrap();
        // After goal 1: end of Jan 4000-2500 = 1500 (<2000); end of Feb 2500 ✓
        assert_eq!(g2.affordable_on, Some(d(2026, 2, 28)));
        assert_eq!(g2.months_away, Some(1));
        assert!((g2.progress - 0.25).abs() < 1e-9);
        // Spendable series reflects both purchases.
        assert_eq!(p.spendable_after_goals[0].1, 500.0);
    }

    #[test]
    fn waits_for_a_later_big_bill() {
        let mut data = base();
        // A $2,500 bill due in month 3 (March).
        data.flows.push(CashFlow {
            id: 2,
            kind: FlowKind::Bill,
            name: "insurance".into(),
            amount: 2500.0,
            frequency: Frequency::Once,
            start_date: d(2026, 3, 10),
            ..income(0.0)
        });
        data.goals.push(goal(1, 3600.0, 1, None));
        let p = plan_goals(&data, d(2026, 1, 1), 2);
        // Spendable: now 3000, end of Jan 4000, Feb 5000, Mar 3500, Apr 4500 ...
        // Without the bill it would be affordable end of January (4000). Buying then would
        // leave -100 after the March bill, so the first safe month is April.
        assert_eq!(p.plan_for(1).unwrap().affordable_on, Some(d(2026, 4, 30)));
    }

    #[test]
    fn baseline_counts_only_cash_accounts_and_target_status() {
        let mut data = base();
        data.accounts.push(Account {
            id: 2,
            kind: AccountKind::Hsa,
            minimum_balance: 9999.0,
            ..checking(10000.0, 0.0)
        });
        data.goals.push(goal(1, 9000.0, 1, Some(d(2026, 3, 1))));
        let p = plan_goals(&data, d(2026, 1, 1), 2);
        assert_eq!(p.baseline, 2000.0); // HSA minimum doesn't affect goals
        let g = p.plan_for(1).unwrap();
        // Needs 6,000 more at $1,000/mo → affordable end of June 2026.
        assert_eq!(g.affordable_on, Some(d(2026, 6, 30)));
        match g.target.clone().unwrap() {
            TargetStatus::Behind { extra_per_month } => {
                // By Mar 1: safe spendable = 5000 (end of Feb) → short 4,000 over 2 months.
                assert!((extra_per_month - 2000.0).abs() < 1e-6, "{extra_per_month}");
            }
            other => panic!("expected behind, got {other:?}"),
        }
    }

    #[test]
    fn bucket_money_is_reserved() {
        let mut data = base();
        // $1,000 of checking set aside as "Rainy day", topped up $200/mo.
        data.buckets.push(Bucket {
            id: 7,
            account_id: 1,
            name: "Rainy day".into(),
            balance: 1000.0,
            monthly_add: 200.0,
            target: None,
            notes: String::new(),
        });
        data.goals.push(goal(1, 2500.0, 1, None));
        let p = plan_goals(&data, d(2026, 1, 1), 2);
        assert_eq!(p.buckets_now, 1000.0);
        assert_eq!(p.spendable_now, 2000.0); // 5000 - 2000 minimum - 1000 bucket
        // Spendable grows 1000 - 200 = 800/mo: end of Jan 2800 ≥ 2500.
        assert_eq!(p.plan_for(1).unwrap().affordable_on, Some(d(2026, 1, 31)));
    }

    #[test]
    fn unaffordable_goal_does_not_block_later_ones_and_bought_goals_are_skipped() {
        let mut data = base();
        data.goals.push(goal(1, 1_000_000.0, 1, None));
        data.goals.push(goal(2, 1000.0, 2, None));
        let mut bought = goal(3, 50.0, 0, None);
        bought.purchased_on = Some(d(2025, 12, 1));
        data.goals.push(bought);
        let p = plan_goals(&data, d(2026, 1, 1), 1);
        assert_eq!(p.plans.len(), 2);
        assert_eq!(p.plan_for(1).unwrap().affordable_on, None);
        assert_eq!(p.plan_for(2).unwrap().affordable_on, Some(d(2026, 1, 1)));
    }
}
