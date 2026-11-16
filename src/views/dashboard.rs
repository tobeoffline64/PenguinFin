use crate::app::{Page, today, use_store};
use crate::ui::badge::*;
use crate::ui::button::*;
use crate::ui::card::*;
use crate::ui::progress::Progress;
use crate::widgets::*;
use dioxus::prelude::*;
use penguinfin::model::{AccountGroup, FlowKind};
use penguinfin::projection::{project, spending_by_category, upcoming};

#[component]
pub fn Dashboard(on_nav: EventHandler<Page>) -> Element {
    let mut store = use_store();
    let data = store.data.read().clone();
    let sym = data.settings.currency_symbol.clone();
    let today = today();

    // The projection runs on a worker thread; the chart shows a skeleton until it's ready.
    let projection = use_resource(move || {
        let data = store.data.read().clone();
        async move {
            let years = data.settings.projection_years.max(1);
            crate::app::off_thread(move || project(&data, crate::app::today(), years)).await
        }
    });

    if data.flows.is_empty() && data.accounts.is_empty() {
        return rsx! {
            PageHeader { title: "Welcome to PenguinFin", subtitle: "Plan bills, recurring expenses, income and investments, all offline." }
            EmptyState {
                title: "No data yet",
                text: "Start by adding your accounts and paychecks, then your bills and recurring expenses. Or load an example household to look around first (you can erase it later in Settings).",
                div { class: "flex gap-2",
                    Button { onclick: move |_| on_nav.call(Page::Accounts), "Add accounts" }
                    Button { variant: ButtonVariant::Outline, onclick: move |_| on_nav.call(Page::Income), "Add income" }
                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| {
                            let d = penguinfin::sample::sample_data(today);
                            store.mutate("Sample data loaded", |db| db.replace_all(&d));
                        },
                        "Load sample data"
                    }
                }
            }
        };
    }

    let income = data.monthly_total(|k| k == FlowKind::Income, today);
    let bills = data.monthly_total(|k| k == FlowKind::Bill, today);
    let expenses = data.monthly_total(|k| k == FlowKind::Expense, today);
    let cash_contrib = data.monthly_cash_contributions();
    let payroll_contrib: f64 =
        data.accounts.iter().filter(|a| !a.contribution_from_cash).map(|a| a.monthly_contribution).sum();
    let match_total: f64 = data.accounts.iter().map(|a| a.employer_match_monthly).sum();
    let debt_plan = data.debt_monthly_budget();
    let debt_total = data.total_debt();
    let leftover = income - bills - expenses - cash_contrib - debt_plan;
    let saved = cash_contrib + leftover.max(0.0);
    let savings_rate = if income > 0.0 { saved / income * 100.0 } else { 0.0 };

    let years = data.settings.projection_years.max(1);
    let proj = projection.read().clone();
    let loading = proj.is_none();
    let proj = proj.unwrap_or_default();
    let yearly = proj.yearly();
    let labels: Vec<String> = yearly.iter().map(|y| y.year.to_string()).collect();
    let nw10 = proj.at_years(10.min(years)).map(|m| m.net_worth()).unwrap_or(0.0);
    let nw_end = proj.months.last().map(|m| m.net_worth()).unwrap_or(0.0);

    // Overview series; a Debt line is added while you owe money.
    let mut overview_series = vec![
        Series { name: "Net worth".into(), color: "var(--chart-3)".into(), values: yearly.iter().map(|y| y.net_worth()).collect(), bold: true },
        Series { name: "Retirement".into(), color: "var(--success)".into(), values: yearly.iter().map(|y| y.retirement).collect(), bold: false },
        Series { name: "Investments".into(), color: "var(--warning)".into(), values: yearly.iter().map(|y| y.investments).collect(), bold: false },
        Series { name: "Cash".into(), color: "var(--chart-1)".into(), values: yearly.iter().map(|y| y.cash).collect(), bold: false },
    ];
    if debt_total > 0.0 {
        overview_series.push(Series {
            name: "Debt".into(),
            color: "var(--destructive)".into(),
            values: yearly.iter().map(|y| y.debt).collect(),
            bold: false,
        });
    }
    // First projected month with nothing owed.
    let debt_free = (debt_total > 0.0 && !loading)
        .then(|| proj.months.iter().find(|m| m.debt <= 0.005).map(|m| m.month))
        .flatten();

    let up = upcoming(&data, today, 30);
    let up_out: f64 = up.iter().filter(|u| u.kind.is_outflow()).map(|u| u.amount).sum();
    let up_in: f64 = up.iter().filter(|u| !u.kind.is_outflow()).map(|u| u.amount).sum();
    let cats = spending_by_category(&data, today);
    let cat_total: f64 = cats.iter().map(|c| c.1).sum();

    rsx! {
        PageHeader { title: "Dashboard", subtitle: format!("As of {}", today.format("%B %-d, %Y")) }

        if let Some(when) = proj.cash_shortfall {
            div { class: "flex items-start gap-3 rounded-xl border border-destructive/50 bg-destructive-light p-4 text-sm text-destructive-dark",
                Icon { name: IconName::Alert, class: "size-5 shrink-0" }
                div {
                    div { class: "font-semibold", "Cash runs short in {when.format(\"%B %Y\")}" }
                    div { "At the current pace, your checking and savings balances would go below zero. Look at bills, expenses or contributions paid from cash." }
                }
            }
        }

        div { class: "grid grid-cols-2 gap-4 @3xl:grid-cols-4",
            Stat { label: "Net worth", value: money(data.net_worth(), &sym),
                sub: format!("Cash {} · Invest {} · Retire {}{}",
                    money_short(data.group_total(AccountGroup::Cash), &sym),
                    money_short(data.group_total(AccountGroup::Investments), &sym),
                    money_short(data.group_total(AccountGroup::Retirement), &sym),
                    if debt_total > 0.0 { format!(" · Debt −{}", money_short(debt_total, &sym)) } else { String::new() }) }
            Stat { label: "Monthly income", value: money(income, &sym), sub: "Average take-home per month" }
            Stat { label: "Bills + expenses", value: money(bills + expenses, &sym),
                sub: format!("Bills {} · Expenses {}", money(bills, &sym), money(expenses, &sym)) }
            Stat { label: "Left over / month", value: money(leftover, &sym),
                tone: if leftover >= 0.0 { "good" } else { "bad" },
                sub: if debt_plan > 0.0 {
                    format!("After {} to savings and {} to debt", money(cash_contrib, &sym), money(debt_plan, &sym))
                } else {
                    format!("After {} moved to savings/investing", money(cash_contrib, &sym))
                } }
            Stat { label: "Savings rate", value: format!("{savings_rate:.0}%"), sub: "Contributions + leftover cash, as % of income" }
            Stat { label: "Paycheck + employer", value: money(payroll_contrib + match_total, &sym), sub: "Monthly into retirement from payroll & match" }
            Stat { label: format!("Net worth in {} yrs", 10.min(years)), value: money(nw10, &sym), sub: "Projected, nominal dollars", loading }
            Stat { label: format!("Net worth in {years} yrs"), value: money(nw_end, &sym), loading,
                sub: format!("{} in today's dollars", money(nw_end / proj.months.last().map(|m| m.inflation_factor).unwrap_or(1.0), &sym)) }
        }

        Card {
            CardHeader {
                CardTitle { "Projected net worth" }
                CardDescription {
                    "Year-end balances using your income, bills, expenses, contributions, debt payments and expected returns."
                    if let Some(d) = debt_free { " Debt-free in {d.format(\"%B %Y\")}." }
                }
            }
            CardContent {
                if loading {
                    ChartSkeleton {}
                } else {
                LineChart {
                    labels: labels.clone(),
                    currency: sym.clone(),
                    series: overview_series.clone(),
                }
                }
            }
        }

        if !data.debts.is_empty() {
            DebtOverview { debt_free, sym: sym.clone() }
        }

        div { class: "grid gap-6 @3xl:grid-cols-5",
            Card { class: "@3xl:col-span-3",
                CardHeader {
                    CardTitle { "Next 30 days" }
                    CardDescription { "In {money(up_in, &sym)} · Out {money(up_out, &sym)}" }
                }
                CardContent { class: "flex flex-col",
                    if up.is_empty() {
                        p { class: "text-sm text-muted-foreground", "Nothing scheduled." }
                    }
                    for u in up.iter().take(14) {
                        div { class: "flex items-center gap-3 border-b py-2 text-sm last:border-0",
                            div { class: "w-16 shrink-0 text-xs text-muted-foreground", "{u.date.format(\"%a %b %-d\")}" }
                            div { class: "flex-1 truncate", "{u.name}" }
                            if u.autopay {
                                Badge { variant: BadgeVariant::Muted, size: BadgeSize::Sm, "autopay" }
                            }
                            match u.kind {
                                FlowKind::Income => rsx! { Badge { variant: BadgeVariant::Success, size: BadgeSize::Sm, "income" } },
                                FlowKind::Bill => rsx! { Badge { variant: BadgeVariant::Warning, size: BadgeSize::Sm, "bill" } },
                                FlowKind::Expense => rsx! { Badge { variant: BadgeVariant::Info, size: BadgeSize::Sm, "expense" } },
                            }
                            div {
                                class: if u.kind.is_outflow() { "w-24 text-right tabular-nums" } else { "w-24 text-right tabular-nums text-success" },
                                {format!("{}{}", if u.kind.is_outflow() { "-" } else { "+" }, money_cents(u.amount, &sym))}
                            }
                        }
                    }
                    if up.len() > 14 {
                        p { class: "pt-2 text-xs text-muted-foreground", "+ {up.len() - 14} more" }
                    }
                }
            }
            Card { class: "@3xl:col-span-2",
                CardHeader {
                    CardTitle { "Monthly spending by category" }
                    CardDescription { "Bills and recurring expenses, monthly average" }
                }
                CardContent { class: "flex flex-col gap-3",
                    if cats.is_empty() {
                        p { class: "text-sm text-muted-foreground", "No bills or expenses yet." }
                    }
                    for (cat, amt) in cats.iter() {
                        div { class: "flex flex-col gap-1",
                            div { class: "flex justify-between text-sm",
                                span { "{cat}" }
                                span { class: "tabular-nums text-muted-foreground", "{money(*amt, &sym)}" }
                            }
                            Progress { value: *amt, max: cat_total.max(1.0) }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn PageHeader(#[props(into)] title: String, #[props(into, optional)] subtitle: Option<String>, children: Element) -> Element {
    rsx! {
        div { class: "flex items-end justify-between gap-4",
            div {
                h1 { class: "text-2xl font-semibold tracking-tight", "{title}" }
                if let Some(s) = subtitle {
                    p { class: "text-sm text-muted-foreground", "{s}" }
                }
            }
            {children}
        }
    }
}

/// Dashboard summary of the debt payoff plan: key numbers and each balance month by month.
#[component]
fn DebtOverview(debt_free: Option<chrono::NaiveDate>, sym: String) -> Element {
    let store = use_store();
    let data = store.data.read().clone();
    let s = &data.settings;
    let p = penguinfin::debt::plan(&data.debts, s.debt_strategy, s.debt_extra_monthly, true, today(), 600);
    let monthly = data.debt_monthly_budget();
    if p.months.is_empty() {
        return rsx! {};
    }
    let n = p.months.len();
    let stride = n.div_ceil(60).max(1);
    let mut idx: Vec<usize> = (0..n).step_by(stride).collect();
    if idx.last() != Some(&(n - 1)) {
        idx.push(n - 1);
    }
    const COLORS: [&str; 6] = ["var(--destructive)", "var(--warning)", "var(--chart-3)", "var(--chart-5)", "var(--info)", "var(--success)"];
    let layers: Vec<Series> = p
        .order
        .iter()
        .rev()
        .enumerate()
        .map(|(k, &i)| Series {
            name: data.debts[i].name.clone(),
            color: COLORS[k % COLORS.len()].into(),
            values: idx.iter().map(|&m| p.months[m].balances[i]).collect(),
            bold: false,
        })
        .collect();
    let total = Series {
        name: "Total owed".into(),
        color: "var(--foreground)".into(),
        values: idx.iter().map(|&m| p.months[m].balances.iter().sum()).collect(),
        bold: true,
    };
    let labels: Vec<String> = idx.iter().map(|&m| p.months[m].month.format("%b %y").to_string()).collect();
    let strategy = match s.debt_strategy {
        penguinfin::debt::Strategy::Avalanche => "avalanche",
        penguinfin::debt::Strategy::Snowball => "snowball",
    };
    rsx! {
        Card {
            CardHeader {
                CardTitle { "Debt payoff" }
                CardDescription {
                    match debt_free.or(p.debt_free) {
                        Some(d) => format!(
                            "Paying {} a month ({strategy}), you're debt-free in {} with {} of interest.",
                            money(monthly, &sym), d.format("%B %Y"), money(p.total_interest, &sym)
                        ),
                        None => format!("At {} a month the debt isn't paid off within 50 years. See Debt payoff.", money(monthly, &sym)),
                    }
                }
            }
            CardContent { class: "flex flex-col gap-4",
                div { class: "grid grid-cols-2 gap-4 @3xl:grid-cols-4",
                    div { class: "flex flex-col",
                        span { class: "text-xs text-muted-foreground", "Owed now" }
                        span { class: "text-lg font-semibold tabular-nums text-destructive", {money(data.total_debt(), &sym)} }
                    }
                    div { class: "flex flex-col",
                        span { class: "text-xs text-muted-foreground", "Paid each month" }
                        span { class: "text-lg font-semibold tabular-nums", {money(monthly, &sym)} }
                    }
                    div { class: "flex flex-col",
                        span { class: "text-xs text-muted-foreground", "Debt-free" }
                        span { class: "text-lg font-semibold tabular-nums text-success",
                            {debt_free.or(p.debt_free).map(|d| d.format("%b %Y").to_string()).unwrap_or_else(|| "—".into())} }
                    }
                    div { class: "flex flex-col",
                        span { class: "text-xs text-muted-foreground", "Interest to pay" }
                        span { class: "text-lg font-semibold tabular-nums", {money(p.total_interest, &sym)} }
                    }
                }
                StackedChart { layers, total, reference: None, labels, currency: sym.clone() }
            }
        }
    }
}
