use crate::app::{today, use_store};
use crate::ui::badge::*;
use crate::ui::button::*;
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::table::*;
use crate::views::dashboard::PageHeader;
use crate::widgets::*;
use chrono::NaiveDate;
use dioxus::prelude::*;
use penguinfin::debt::{Debt, DebtKind, PayoffPlan, Strategy, payment_for_months, plan};
use penguinfin::model::FlowKind;

/// Plans run up to 50 years; anything longer is reported as "not paid off".
const MAX_MONTHS: u32 = 600;

fn months_text(n: usize) -> String {
    match n {
        0 => "this month".into(),
        1 => "1 month".into(),
        m if m < 24 => format!("{m} months"),
        m => {
            let (y, r) = (m / 12, m % 12);
            if r == 0 { format!("{y} years") } else { format!("{y} yr {r} mo") }
        }
    }
}

fn month_label(d: NaiveDate) -> String {
    d.format("%b %Y").to_string()
}

#[derive(Clone, PartialEq)]
struct Plans {
    chosen: PayoffPlan,
    minimums_only: PayoffPlan,
}

#[component]
pub fn DebtsPage() -> Element {
    let mut store = use_store();
    let data = store.data.read().clone();
    let sym = data.settings.currency_symbol.clone();
    let today = today();
    let mut form = use_signal(DebtForm::new);
    let mut generation = use_signal(|| 0u32);
    let editing_id = use_memo(move || form.read().id);

    // Plans run on a worker thread; skeletons show while they're calculated.
    let plans = use_resource(move || {
        let data = store.data.read().clone();
        async move {
            crate::app::off_thread(move || {
                let s = &data.settings;
                Plans {
                    chosen: plan(&data.debts, s.debt_strategy, s.debt_extra_monthly, true, today, MAX_MONTHS),
                    minimums_only: plan(&data.debts, s.debt_strategy, 0.0, false, today, MAX_MONTHS),
                }
            })
            .await
        }
    });
    // Right after a debt is added or removed the last plan is for the old list of debts;
    // treat it as still loading until the new one is ready (indexes must line up).
    let plans_value = plans.read().clone().filter(|p| p.chosen.payoff.len() == data.debts.len());
    let loading = plans_value.is_none();

    // Monthly budget: what's left after rent, utilities and other bills, expenses and savings.
    let income = data.monthly_total(|k| k == FlowKind::Income, today);
    let bills = data.monthly_total(|k| k == FlowKind::Bill, today);
    let expenses = data.monthly_total(|k| k == FlowKind::Expense, today);
    let saving = data.monthly_cash_contributions();
    let available = income - bills - expenses - saving;
    let active: Vec<&Debt> = data.debts.iter().filter(|d| d.balance > 0.0).collect();
    let minimums: f64 = active.iter().map(|d| d.min_payment.max(0.0)).sum();
    let extra = data.settings.debt_extra_monthly.max(0.0);
    let after_plan = available - minimums - extra;
    let interest_now: f64 = data.debts.iter().map(Debt::monthly_interest).sum();
    let strategy = data.settings.debt_strategy;
    let suggestion = (available - minimums).max(0.0);
    // The debt that currently gets the extra payment: first in plan order still owed.
    let target = plan_order(&data.debts, plans_value.as_ref()).into_iter().find(|&i| data.debts[i].balance > 0.0);

    let mut set_extra = move |v: f64| {
        let mut s = store.data.read().settings.clone();
        s.debt_extra_monthly = (v.max(0.0) * 100.0).round() / 100.0;
        store.mutate("", |db| db.save_settings(&s));
    };
    let mut set_strategy = move |st: Strategy| {
        let mut s = store.data.read().settings.clone();
        s.debt_strategy = st;
        store.mutate("", |db| db.save_settings(&s));
    };

    rsx! {
        PageHeader { title: "Debt payoff",
            subtitle: "Credit cards and loans: what they cost, and a plan to pay them off while still covering rent and your bills." }

        div { class: "grid grid-cols-2 gap-4 @3xl:grid-cols-4",
            Stat { label: "Total debt", value: money(data.total_debt(), &sym),
                sub: format!("{} debt{}", active.len(), if active.len() == 1 { "" } else { "s" }),
                tone: if data.total_debt() > 0.0 { "bad" } else { "good" } }
            Stat { label: "Interest this month", value: money_cents(interest_now, &sym),
                sub: format!("About {} a year at today's balances", money(interest_now * 12.0, &sym)) }
            match &plans_value {
                None => rsx! { Stat { label: "Debt-free", value: "", loading: true } },
                Some(p) => match p.chosen.debt_free {
                    _ if active.is_empty() => rsx! { Stat { label: "Debt-free", value: "Now", tone: "good", sub: "No debt to pay off" } },
                    Some(d) => rsx! { Stat { label: "Debt-free", value: month_label(d), tone: "good",
                        sub: format!("In {} with this plan", months_text(p.chosen.months.len())) } },
                    None => rsx! { Stat { label: "Debt-free", value: "Not on this plan", tone: "bad",
                        sub: "Payments don't cover the interest. Raise the extra payment." } },
                },
            }
            match &plans_value {
                None => rsx! { Stat { label: "Interest saved", value: "", loading: true } },
                Some(p) => rsx! {
                    Stat { label: "Total interest",
                        value: money(p.chosen.total_interest, &sym),
                        sub: match (p.minimums_only.debt_free, p.chosen.debt_free) {
                            (Some(_), Some(_)) => format!(
                                "Saves {} and {} vs. minimums only",
                                money((p.minimums_only.total_interest - p.chosen.total_interest).max(0.0), &sym),
                                months_text(p.minimums_only.months.len().saturating_sub(p.chosen.months.len()))
                            ),
                            (None, Some(_)) => "Minimums alone would never pay it all off".to_string(),
                            _ => String::new(),
                        },
                    }
                },
            }
        }

        if !data.debts.is_empty() {
            Card {
                CardHeader {
                    CardTitle { "Your monthly budget" }
                    CardDescription { "Rent, utilities and your other bills come first. The plan only uses what's left." }
                }
                CardContent { class: "grid gap-6 @3xl:grid-cols-2",
                    div { class: "flex flex-col gap-1 text-sm",
                        BudgetLine { label: "Take-home income", amount: income, sym: sym.clone(), sign: "+" }
                        BudgetLine { label: "Bills (rent, utilities, insurance…)", amount: bills, sym: sym.clone(), sign: "−" }
                        BudgetLine { label: "Recurring expenses", amount: expenses, sym: sym.clone(), sign: "−" }
                        if saving > 0.0 {
                            BudgetLine { label: "Savings & investing from checking", amount: saving, sym: sym.clone(), sign: "−" }
                        }
                        div { class: "my-1 border-t" }
                        BudgetLine { label: "Available for debt", amount: available, sym: sym.clone(), sign: "=", strong: true }
                        BudgetLine { label: "Minimum payments", amount: minimums, sym: sym.clone(), sign: "−" }
                        BudgetLine { label: "Extra toward debt", amount: extra, sym: sym.clone(), sign: "−" }
                        div { class: "my-1 border-t" }
                        BudgetLine { label: "Left over after the plan", amount: after_plan, sym: sym.clone(), sign: "=", strong: true,
                            tone: if after_plan < 0.0 { "bad" } else { "good" } }
                        if available < minimums {
                            p { class: "mt-2 rounded-md bg-destructive-light px-3 py-2 text-xs text-destructive-dark",
                                "Your minimum payments are {money(minimums - available, &sym)} more than what's left after bills and expenses. Look for expenses to cut, or ask lenders about lower payments." }
                        } else if after_plan < 0.0 {
                            p { class: "mt-2 rounded-md bg-warning-light px-3 py-2 text-xs text-warning-dark",
                                "This plan needs {money(-after_plan, &sym)} more per month than you have after rent, bills and expenses. Lower the extra payment to at most {money(suggestion, &sym)}." }
                        }
                    }
                    div { class: "flex flex-col gap-4",
                        Field { label: "Extra toward debt each month ({sym})",
                            hint: format!("On top of the minimums. Up to {} keeps every bill covered.", money(suggestion, &sym)),
                            div { class: "flex gap-2",
                                for v in [num_str(extra)] {
                                    Input { key: "{v}", value: v.clone(), placeholder: "0",
                                        oninput: move |e: FormEvent| if let Some(x) = parse_num(&e.value()) { set_extra(x) } }
                                }
                                Button { variant: ButtonVariant::Outline, title: "Use everything left after bills, expenses and minimums",
                                    onclick: move |_| set_extra(suggestion.floor()), "Use {money(suggestion.floor(), &sym)}" }
                            }
                        }
                        div { class: "flex flex-col gap-2",
                            span { class: "text-xs font-medium text-muted-foreground", "Which debt gets the extra first" }
                            div { class: "grid grid-cols-2 gap-2",
                                StrategyButton { selected: strategy == Strategy::Avalanche, title: "Avalanche",
                                    text: "Highest interest rate first. Pays the least interest.",
                                    onclick: move |_| set_strategy(Strategy::Avalanche) }
                                StrategyButton { selected: strategy == Strategy::Snowball, title: "Snowball",
                                    text: "Smallest balance first. Quick wins keep you motivated.",
                                    onclick: move |_| set_strategy(Strategy::Snowball) }
                            }
                            p { class: "text-xs text-muted-foreground",
                                "When a debt is paid off, its payment rolls into the next one, so the monthly total stays the same until you're debt-free." }
                        }
                    }
                }
            }
        }

        div { class: "grid gap-6 @5xl:grid-cols-3",
            div { class: "flex flex-col gap-6 @5xl:col-span-2",
                if data.debts.is_empty() {
                    EmptyState { title: "No debts",
                        text: "Add credit cards and loans to see what they cost each month and get a plan to pay them off." }
                } else {
                    match &plans_value {
                        None => rsx! { Card { CardContent { ChartSkeleton {} } } },
                        Some(p) => rsx! { PayoffChart { plan: p.chosen.clone(), sym: sym.clone() } },
                    }
                    Card {
                        CardHeader {
                            CardTitle { "Debts" }
                            CardDescription { "In payoff order. The first one still owed gets the extra payment." }
                        }
                        CardContent { class: "flex flex-col gap-3",
                            for (pos, idx) in plan_order(&data.debts, plans_value.as_ref()).into_iter().enumerate() {
                                DebtCard {
                                    key: "{data.debts[idx].id}",
                                    debt: data.debts[idx].clone(),
                                    position: pos + 1,
                                    plans: plans_value.clone(),
                                    index: idx,
                                    loading,
                                    selected: editing_id() == data.debts[idx].id,
                                    is_target: target == Some(idx),
                                    sym: sym.clone(),
                                    pay_from: data.debts[idx].account_id.and_then(|a| data.account_name(a)).map(str::to_string)
                                        .unwrap_or_else(|| data.primary_cash_account().map(|a| a.name.clone()).unwrap_or_default()),
                                    set_aside: data.buckets.iter().find(|b| b.debt_id == Some(data.debts[idx].id))
                                        .map(|b| (b.balance, data.account_name(b.account_id).unwrap_or("").to_string())),
                                    on_edit: move |d: Debt| {
                                        let has_bucket = store.data.read().buckets.iter().any(|b| b.debt_id == Some(d.id));
                                        form.set(DebtForm::from_debt(&d, has_bucket));
                                        generation += 1;
                                    },
                                }
                            }
                        }
                    }
                    if let Some(p) = &plans_value {
                        Schedule { plan: p.chosen.clone(), debts: data.debts.clone(), sym: sym.clone() }
                    }
                }
            }
            DebtFormCard { form, generation, sym: sym.clone() }
        }
    }
}

/// Debt indices in plan order (falls back to entry order while the plan is loading).
fn plan_order(debts: &[Debt], plans: Option<&Plans>) -> Vec<usize> {
    match plans {
        Some(p) if p.chosen.order.len() == debts.len() => p.chosen.order.clone(),
        _ => (0..debts.len()).collect(),
    }
}

#[component]
fn BudgetLine(
    #[props(into)] label: String,
    amount: f64,
    sym: String,
    #[props(into)] sign: String,
    #[props(default)] strong: bool,
    #[props(into, default)] tone: String,
) -> Element {
    let color = match tone.as_str() {
        "good" => "text-success",
        "bad" => "text-destructive",
        _ => "",
    };
    let weight = if strong { "font-semibold" } else { "text-muted-foreground" };
    rsx! {
        div { class: "flex items-center justify-between gap-3 {weight}",
            span { "{label}" }
            span { class: "tabular-nums {color}", "{sign} {money(amount.abs(), &sym)}" }
        }
    }
}

#[component]
fn StrategyButton(selected: bool, #[props(into)] title: String, #[props(into)] text: String, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button {
            class: if selected { "flex flex-col items-start gap-1 rounded-lg border-2 border-primary p-3 text-left" }
                   else { "flex flex-col items-start gap-1 rounded-lg border p-3 text-left hover:bg-accent cursor-pointer" },
            onclick: move |e| onclick.call(e),
            span { class: "text-sm font-semibold", "{title}" }
            span { class: "text-xs text-muted-foreground", "{text}" }
        }
    }
}

#[component]
fn DebtCard(
    debt: Debt,
    position: usize,
    plans: Option<Plans>,
    index: usize,
    loading: bool,
    selected: bool,
    is_target: bool,
    /// (amount set aside now, account name) when the debt has a payment bucket.
    set_aside: Option<(f64, String)>,
    sym: String,
    pay_from: String,
    on_edit: EventHandler<Debt>,
) -> Element {
    let mut store = use_store();
    let mut confirm = use_signal(|| false);
    let id = debt.id;
    let d2 = debt.clone();
    let border = if selected { "rounded-lg border border-ring p-4" } else { "rounded-lg border p-4" };
    let interest = debt.monthly_interest();
    let covers_interest = debt.min_payment > interest + 0.005 || debt.balance <= 0.0;
    let fixed: Vec<(u32, f64)> = [12u32, 24, 36].iter().map(|&m| (m, payment_for_months(debt.balance, debt.apr, m))).collect();
    rsx! {
        div { class: "{border} flex flex-col gap-3",
            div { class: "flex items-start gap-3",
                div { class: "flex size-7 shrink-0 items-center justify-center rounded-full bg-secondary text-xs font-semibold", "{position}" }
                div { class: "flex min-w-0 flex-1 flex-col gap-1",
                    div { class: "flex flex-wrap items-center gap-2",
                        span { class: "font-medium", "{debt.name}" }
                        Badge { variant: BadgeVariant::Secondary, size: BadgeSize::Sm, "{debt.kind.label()}" }
                        if is_target {
                            Badge { variant: BadgeVariant::Info, size: BadgeSize::Sm, "Gets the extra payment" }
                        }
                    }
                    div { class: "text-xs text-muted-foreground",
                        "{num_str(debt.apr)}% APR · minimum {money_cents(debt.min_payment, &sym)}/mo · from {pay_from}" }
                }
                div { class: "text-right",
                    div { class: "text-lg font-semibold tabular-nums", {money(debt.balance, &sym)} }
                    div { class: "text-xs text-muted-foreground tabular-nums", "{money_cents(interest, &sym)} interest/mo" }
                }
                div { class: "flex shrink-0",
                    if confirm() {
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Destructive,
                            onclick: move |_| store.mutate("Debt removed", |db| db.delete_debt(id)), "Delete" }
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| confirm.set(false), "Keep" }
                    } else {
                        Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Edit (e.g. update the balance)",
                            onclick: move |_| on_edit.call(d2.clone()), Icon { name: IconName::Pencil } }
                        Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Delete",
                            onclick: move |_| confirm.set(true), Icon { name: IconName::Trash } }
                    }
                }
            }
            if !covers_interest {
                p { class: "rounded-md bg-destructive-light px-3 py-2 text-xs text-destructive-dark",
                    "The minimum payment doesn't cover the {money_cents(interest, &sym)} of interest charged each month, so this balance keeps growing unless you pay more." }
            }
            if loading {
                LoadingBlock { class: "h-10 w-full" }
            } else if let Some(p) = &plans {
                div { class: "grid gap-2 text-xs @2xl:grid-cols-2",
                    div { class: "rounded-md bg-muted/60 px-3 py-2",
                        div { class: "font-medium text-foreground", "With your plan" }
                        match p.chosen.payoff[index] {
                            Some(d) => rsx! { div { "Paid off {month_label(d)} · interest {money(p.chosen.interest_by_debt[index], &sym)}" } },
                            None => rsx! { div { class: "text-destructive", "Not paid off within 50 years" } },
                        }
                    }
                    div { class: "rounded-md bg-muted/60 px-3 py-2",
                        div { class: "font-medium text-foreground", "Paying only the minimum" }
                        match p.minimums_only.payoff[index] {
                            Some(d) => rsx! { div { "Paid off {month_label(d)} · interest {money(p.minimums_only.interest_by_debt[index], &sym)}" } },
                            None => rsx! { div { class: "text-destructive", "Never paid off" } },
                        }
                    }
                }
                div { class: "flex flex-wrap items-center gap-2 text-xs",
                    match &set_aside {
                        Some((amount, account)) => rsx! {
                            span { class: "text-muted-foreground",
                                "Set aside: ", b { class: "text-foreground", {money_cents(*amount, &sym)} }
                                " this month in {account} › {debt.name} payment (follows your plan)" }
                        },
                        None if debt.balance > 0.0 => rsx! {
                            Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline,
                                title: "Create a bucket in the paying account that holds this debt's planned payment",
                                onclick: move |_| store.mutate("Payments now set aside in a bucket", |db| db.add_debt_bucket(id)),
                                "Set aside payments in a bucket" }
                        },
                        None => rsx! {},
                    }
                }
                if debt.balance > 0.0 {
                    div { class: "flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground",
                        span { "Monthly payment to be debt-free in:" }
                        for (m, pay) in fixed.iter() {
                            span { key: "{m}", class: "tabular-nums", "{m / 12} yr: ", b { class: "text-foreground", {money(*pay, &sym)} } }
                        }
                    }
                }
            }
        }
    }
}

/// Each debt's balance until debt-free, stacked.
#[component]
fn PayoffChart(plan: PayoffPlan, sym: String) -> Element {
    if plan.months.is_empty() {
        return rsx! {};
    }
    let n = plan.months.len();
    // Up to ~120 points: monthly for short plans, every few months for long ones.
    let stride = n.div_ceil(120).max(1);
    let mut idx: Vec<usize> = (0..n).step_by(stride).collect();
    if idx.last() != Some(&(n - 1)) {
        idx.push(n - 1);
    }
    let labels: Vec<String> = idx.iter().map(|&i| plan.months[i].month.format("%b %y").to_string()).collect();
    let store = use_store();
    let debts = store.data.read().debts.clone();
    const COLORS: [&str; 6] = ["var(--destructive)", "var(--warning)", "var(--chart-3)", "var(--chart-5)", "var(--info)", "var(--success)"];
    let layers: Vec<Series> = plan
        .order
        .iter()
        .rev()
        .enumerate()
        .filter(|(_, i)| **i < debts.len())
        .map(|(n, &i)| Series {
            name: debts[i].name.clone(),
            color: COLORS[n % COLORS.len()].into(),
            values: idx.iter().map(|&k| plan.months[k].balances[i]).collect(),
            bold: false,
        })
        .collect();
    let total = Series {
        name: "Total owed".into(),
        color: "var(--foreground)".into(),
        values: idx.iter().map(|&k| plan.months[k].balances.iter().sum()).collect(),
        bold: true,
    };
    rsx! {
        Card {
            CardHeader {
                CardTitle { "Payoff plan" }
                CardDescription {
                    match plan.debt_free {
                        Some(d) => format!("Debt-free in {}, with {} of interest along the way.", month_label(d), money(plan.total_interest, &sym)),
                        None => "At these payments the debt isn't paid off within 50 years.".to_string(),
                    }
                }
            }
            CardContent { StackedChart { layers, total, reference: None, labels, currency: sym } }
        }
    }
}

/// Month-by-month payments.
#[component]
fn Schedule(plan: PayoffPlan, debts: Vec<Debt>, sym: String) -> Element {
    let mut show_all = use_signal(|| false);
    let n = plan.months.len();
    let shown = if show_all() { n } else { n.min(12) };
    rsx! {
        Card {
            CardHeader {
                CardTitle { "Payment schedule" }
                CardDescription { "What to pay each month. Each payment comes from the account chosen for that debt." }
            }
            CardContent { class: "flex flex-col gap-3",
                TableWrapper { class: "max-h-[30rem]",
                    Table {
                        TableHeader {
                            TableRow {
                                TableHead { "Month" }
                                for d in debts.iter() { TableHead { key: "{d.id}", class: "text-right", "{d.name}" } }
                                TableHead { class: "text-right", "Interest" }
                                TableHead { class: "text-right", "Still owed" }
                            }
                        }
                        TableBody {
                            for (k, m) in plan.months.iter().take(shown).enumerate() {
                                TableRow { key: "{k}",
                                    TableCell { class: "whitespace-nowrap font-medium", {month_label(m.month)} }
                                    for (i, d) in debts.iter().enumerate() {
                                        TableCell { key: "{d.id}",
                                            class: if plan.payoff[i] == Some(m.month) { "text-right tabular-nums font-semibold text-success" } else { "text-right tabular-nums" },
                                            if m.payments.get(i).copied().unwrap_or(0.0) > 0.005 {
                                                {money_cents(m.payments[i], &sym)}
                                                if plan.payoff[i] == Some(m.month) { " ✓" }
                                            } else { "—" }
                                        }
                                    }
                                    TableCell { class: "text-right tabular-nums text-muted-foreground", {money_cents(m.interest.iter().sum(), &sym)} }
                                    TableCell { class: "text-right tabular-nums", {money(m.balances.iter().sum(), &sym)} }
                                }
                            }
                        }
                    }
                }
                if n > 12 {
                    div {
                        Button { variant: ButtonVariant::Link, size: ButtonSize::Sm, class: "px-0",
                            onclick: move |_| show_all.toggle(),
                            if show_all() { "Show the first 12 months" } else { "Show all {n} months" } }
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct DebtForm {
    id: i64,
    name: String,
    kind: DebtKind,
    balance: String,
    apr: String,
    min_payment: String,
    account: String,
    notes: String,
    /// Keep this debt's payment set aside in a bucket of the paying account.
    set_aside: bool,
}

impl DebtForm {
    fn new() -> Self {
        DebtForm {
            id: 0,
            name: String::new(),
            kind: DebtKind::CreditCard,
            balance: String::new(),
            apr: String::new(),
            min_payment: String::new(),
            account: String::new(),
            notes: String::new(),
            set_aside: true,
        }
    }
    fn from_debt(d: &Debt, has_bucket: bool) -> Self {
        DebtForm {
            id: d.id,
            name: d.name.clone(),
            kind: d.kind,
            balance: num_str(d.balance),
            apr: num_str(d.apr),
            min_payment: num_str(d.min_payment),
            account: d.account_id.map(|a| a.to_string()).unwrap_or_default(),
            notes: d.notes.clone(),
            set_aside: has_bucket,
        }
    }
    fn to_debt(&self) -> Result<Debt, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Give the debt a name, e.g. Visa card.".into());
        }
        let balance = parse_num(&self.balance).filter(|v| *v >= 0.0).ok_or("Enter how much you owe.")?;
        let apr = if self.apr.trim().is_empty() { 0.0 } else { parse_num(&self.apr).filter(|v| (0.0..=100.0).contains(v)).ok_or("APR must be between 0 and 100.")? };
        let min_payment = parse_num(&self.min_payment).filter(|v| *v >= 0.0).ok_or("Enter the minimum monthly payment (it's on your statement).")?;
        Ok(Debt {
            id: self.id,
            name: name.into(),
            kind: self.kind,
            balance,
            apr,
            min_payment,
            account_id: self.account.parse().ok(),
            notes: self.notes.trim().into(),
        })
    }
}

#[component]
fn DebtFormCard(form: Signal<DebtForm>, generation: Signal<u32>, sym: String) -> Element {
    let mut store = use_store();
    let (mut form, mut generation) = (form, generation);
    let mut error = use_signal(|| None::<String>);
    let f = form.read().clone();
    let editing = f.id != 0;
    let data = store.data.read().clone();
    let kind_options: Vec<(String, String)> = DebtKind::ALL.iter().map(|k| (k.key().to_string(), k.label().to_string())).collect();
    let mut account_options = vec![(
        String::new(),
        match data.primary_cash_account() {
            Some(a) => format!("Default ({})", a.name),
            None => "Default".into(),
        },
    )];
    account_options.extend(data.accounts.iter().map(|a| (a.id.to_string(), a.name.clone())));
    let set_aside_hint = if f.name.trim().is_empty() {
        "Adds a payment bucket to the paying account that always holds this debt's planned payment, so goals and spending won't use it.".to_string()
    } else {
        format!(
            "Adds a \u{201c}{} payment\u{201d} bucket to the paying account that always holds this debt's planned payment, so goals and spending won't use it.",
            f.name.trim()
        )
    };
    // Live preview from what's typed so far.
    let preview = f.to_debt().ok().filter(|d| d.balance > 0.0);

    let save = move |_| {
        let res = form.read().to_debt();
        match res {
            Ok(d) => {
                error.set(None);
                let set_aside = form.read().set_aside;
                let msg = match (d.id == 0, set_aside) {
                    (true, true) => "Debt added, with a bucket for its payments",
                    (true, false) => "Debt added",
                    _ => "Debt saved",
                };
                store.mutate(msg, |db| {
                    let id = db.save_debt(&d)?;
                    if set_aside { db.add_debt_bucket(id) } else { db.remove_debt_bucket(id) }
                });
                form.set(DebtForm::new());
                generation += 1;
            }
            Err(e) => error.set(Some(e)),
        }
    };

    rsx! {
        Card {
            CardHeader {
                CardTitle { if editing { "Edit debt" } else { "Add a debt" } }
                CardDescription { "Find the balance, APR and minimum payment on your latest statement." }
            }
            for g in [generation()] {
            CardContent { key: "{g}", class: "flex flex-col gap-4",
                Field { label: "Type",
                    NativeSelect { value: f.kind.key().to_string(), options: kind_options.clone(),
                        onchange: move |v: String| if let Some(k) = DebtKind::from_key(&v) { form.write().kind = k } }
                }
                Field { label: "Name",
                    Input { value: f.name.clone(), placeholder: "e.g. Visa card",
                        oninput: move |e: FormEvent| form.write().name = e.value() }
                }
                Field { label: "Balance owed ({sym})",
                    Input { value: f.balance.clone(), placeholder: "0",
                        oninput: move |e: FormEvent| form.write().balance = e.value() }
                }
                div { class: "grid grid-cols-2 items-end gap-3",
                    Field { label: "Interest rate (APR %)",
                        Input { value: f.apr.clone(), placeholder: "e.g. 24.99",
                            oninput: move |e: FormEvent| form.write().apr = e.value() }
                    }
                    Field { label: "Minimum payment ({sym}/mo)",
                        Input { value: f.min_payment.clone(), placeholder: "0",
                            oninput: move |e: FormEvent| form.write().min_payment = e.value() }
                    }
                }
                Field { label: "Paid from",
                    NativeSelect { value: f.account.clone(), options: account_options.clone(),
                        onchange: move |v: String| form.write().account = v }
                }
                if let Some(d) = preview.clone() {
                    div { class: "rounded-md bg-muted/60 px-3 py-2 text-xs text-muted-foreground",
                        "Interest this month: ", b { class: "text-foreground", {money_cents(d.monthly_interest(), &sym)} }
                        br {}
                        {
                            let only_min = plan(std::slice::from_ref(&d), Strategy::Avalanche, 0.0, false, today(), MAX_MONTHS);
                            match only_min.debt_free {
                                Some(free) => format!("At the minimum: paid off {} ({}), {} interest.", month_label(free), months_text(only_min.months.len()), money(only_min.total_interest, &sym)),
                                None => "At the minimum this is never paid off.".to_string(),
                            }
                        }
                    }
                }
                div { class: "flex items-start justify-between gap-3",
                    div { class: "flex flex-col",
                        span { class: "text-sm", "Set aside payments in a bucket" }
                        span { class: "text-xs text-muted-foreground", "{set_aside_hint}" }
                    }
                    crate::ui::switch::Switch { checked: f.set_aside, aria_label: "Set aside payments in a bucket",
                        onchange: move |v| form.write().set_aside = v }
                }
                Field { label: "Notes",
                    Input { value: f.notes.clone(), placeholder: "Optional", oninput: move |e: FormEvent| form.write().notes = e.value() }
                }
                if let Some(e) = error() {
                    p { class: "text-sm text-destructive", "{e}" }
                }
                div { class: "flex gap-2",
                    Button { onclick: save, Icon { name: IconName::Plus } if editing { "Save changes" } else { "Add debt" } }
                    if editing {
                        Button { variant: ButtonVariant::Outline,
                            onclick: move |_| { error.set(None); form.set(DebtForm::new()); generation += 1; }, "Cancel" }
                    }
                }
                p { class: "text-xs text-muted-foreground",
                    "Update the balance from your statement now and then; the plan recalculates from there." }
            }
            }
        }
    }
}
