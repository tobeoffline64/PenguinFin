//! Example household used for the "Load sample data" button and tests.

use crate::model::*;
use chrono::{Datelike, NaiveDate};

pub fn sample_data(today: NaiveDate) -> AppData {
    let y = today.year();
    let m = today.month();
    let on = |day: u32| add_months_clamped(NaiveDate::from_ymd_opt(y, m, 1).unwrap(), 0, day).unwrap();
    let flow = |kind, name: &str, cat: &str, amount, frequency, start, change, autopay| CashFlow {
        id: 0,
        kind,
        name: name.into(),
        category: cat.into(),
        amount,
        frequency,
        start_date: start,
        end_date: None,
        annual_change_pct: change,
        autopay,
        notes: String::new(),
    };
    use FlowKind::*;
    use Frequency::*;
    let mut flows = vec![
        flow(Income, "Paycheck (net)", "Salary", 2650.0, Biweekly, on(7), 3.0, true),
        flow(Income, "Side work", "Freelance", 400.0, Monthly, on(20), 0.0, false),
        flow(Bill, "Rent / Mortgage", "Housing", 1850.0, Monthly, on(1), 0.0, true),
        flow(Bill, "Electric", "Utilities", 120.0, Monthly, on(18), 3.0, true),
        flow(Bill, "Internet", "Utilities", 70.0, Monthly, on(22), 0.0, true),
        flow(Bill, "Phone", "Utilities", 85.0, Monthly, on(12), 0.0, true),
        flow(Bill, "Car payment", "Transportation", 410.0, Monthly, on(15), 0.0, false),
        flow(Bill, "Car insurance", "Insurance", 640.0, SemiAnnually, on(25), 4.0, false),
        flow(Expense, "Groceries", "Food", 160.0, Weekly, on(3), 3.0, false),
        flow(Expense, "Gas", "Transportation", 55.0, Weekly, on(5), 3.0, false),
        flow(Expense, "Streaming", "Subscriptions", 23.0, Monthly, on(9), 0.0, true),
        flow(Expense, "Dining out", "Food", 220.0, Monthly, on(28), 3.0, false),
        flow(Expense, "Gifts & holidays", "Personal", 900.0, Annually, on(1), 3.0, false),
    ];
    // The car loan ends in 3 years.
    flows[6].end_date = NaiveDate::from_ymd_opt(y + 3, m, 15);

    let acct = |id, name: &str, kind: AccountKind, balance, ret, contrib, from_cash, matched| Account {
        id,
        name: name.into(),
        kind,
        balance,
        annual_return_pct: ret,
        monthly_contribution: contrib,
        contribution_from_cash: from_cash,
        employer_match_monthly: matched,
        notes: String::new(),
    };
    let accounts = vec![
        acct(1, "Checking", AccountKind::Checking, 4200.0, 0.0, 0.0, false, 0.0),
        acct(2, "High-yield savings", AccountKind::Savings, 12000.0, 4.0, 300.0, true, 0.0),
        acct(3, "Brokerage", AccountKind::Brokerage, 150.0, 7.0, 250.0, true, 0.0),
        acct(4, "Work 401(k)", AccountKind::Retirement401k, 48000.0, 7.0, 600.0, false, 300.0),
        acct(5, "Roth IRA", AccountKind::RothIra, 21000.0, 7.0, 500.0, true, 0.0),
    ];
    let hold = |id, account_id, symbol: &str, shares, price| Holding {
        id,
        account_id,
        symbol: symbol.into(),
        shares,
        price,
        price_updated: Some(today),
    };
    let holdings = vec![hold(1, 3, "VTI", 30.0, 290.0), hold(2, 3, "VXUS", 40.0, 65.0)];
    AppData { flows, accounts, holdings, settings: Settings::default() }
}
