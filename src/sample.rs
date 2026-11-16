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
        account_id: None,
        bucket_id: None,
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
        purpose: String::new(),
        minimum_balance: 0.0,
    };
    let accounts = vec![
        acct(1, "Checking", AccountKind::Checking, 4200.0, 0.0, 0.0, false, 0.0),
        acct(2, "High-yield savings", AccountKind::Savings, 12000.0, 4.0, 450.0, true, 0.0),
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
    // Savings holds two buckets; car costs are paid from savings.
    let bucket = |id, name: &str, balance, monthly_add, target| Bucket {
        id,
        account_id: 2,
        name: name.into(),
        balance,
        monthly_add,
        target,
        notes: String::new(),
    };
    let buckets = vec![bucket(1, "Rainy day", 5000.0, 150.0, Some(10000.0)), bucket(2, "Car repairs", 600.0, 85.0, None)];
    flows.push(CashFlow {
        bucket_id: Some(2),
        ..flow(Bill, "Car maintenance", "Transportation", 450.0, SemiAnnually, on(10), 3.0, false)
    });
    flows[7].account_id = Some(2); // car insurance comes out of savings
    // Medical comes out of savings too.
    flows.push(CashFlow { account_id: Some(2), ..flow(Expense, "Doctor & pharmacy", "Health", 60.0, Monthly, on(14), 4.0, false) });
    let mut accounts = accounts;
    accounts[0].purpose = "Everyday spending".into();
    accounts[0].minimum_balance = 1500.0;
    accounts[1].purpose = "Emergency fund".into();
    accounts[1].minimum_balance = 1000.0;
    let goal = |id, name: &str, cost, priority, target_date| Goal {
        id,
        name: name.into(),
        cost,
        target_date,
        priority,
        notes: String::new(),
        purchased_on: None,
    };
    let goals = vec![
        goal(1, "New laptop", 1800.0, 1, None),
        goal(2, "Family vacation", 4500.0, 2, add_months_clamped(today, 8, 1)),
        goal(3, "Kitchen remodel", 15000.0, 3, None),
    ];
    use crate::debt::{Debt, DebtKind};
    let debt = |id, name: &str, kind, balance, apr, min_payment| Debt {
        id,
        name: name.into(),
        kind,
        balance,
        apr,
        min_payment,
        account_id: None,
        notes: String::new(),
    };
    let debts = vec![
        debt(1, "Visa card", DebtKind::CreditCard, 3200.0, 24.99, 95.0),
        debt(2, "Store card", DebtKind::CreditCard, 650.0, 29.99, 35.0),
        debt(3, "Student loan", DebtKind::StudentLoan, 14000.0, 5.5, 160.0),
    ];
    let settings = Settings { debt_extra_monthly: 250.0, ..Settings::default() };
    AppData { flows, accounts, holdings, buckets, goals, debts, settings, ..Default::default() }
}
