//! The dashboard: headline numbers, projected net worth, the debt plan, the next 30 days and
//! spending by category. After an upgrade from PenguinFin 1 it also says where the copy is.

use crate::app::{Page, use_store};
use crate::charts::{LineChart, Series, StackedChart};
use crate::pages::debts::plans_for;
use crate::ui::icon::{Icon, IconName};
use crate::ui::*;
use chrono::NaiveDate;
use leptos::prelude::*;
use penguinfin_core::debt::Strategy;
use penguinfin_core::format::{money, money_cents, money_short};
use penguinfin_core::model::{AccountGroup, FlowKind};
use penguinfin_core::projection::{project, spending_by_category, upcoming};
use serde::Serialize;

#[derive(Serialize)]
struct KindArg {
    kind: &'static str,
}

#[component]
pub fn Overview(#[prop(into)] on_nav: Callback<Page>) -> impl IntoView {
    let store = use_store();
    let today = store.today();
    let projection = Memo::new(move |_| {
        let data = store.data();
        project(&data, today, data.settings.projection_years.max(1))
    });
    let upgraded = move || {
        store.upgraded.get().map(|copy| view! {
            <div class="flex flex-col gap-2 rounded-lg border bg-muted/40 p-4 text-sm">
                <div class="font-medium">"Your PenguinFin 1 data was upgraded"</div>
                <p class="text-muted-foreground">"Check the numbers below against PenguinFin 1. A copy of your data from before the upgrade was saved at:"</p>
                <code class="rounded bg-muted px-2 py-1 text-xs break-all">{copy}</code>
                <div><Button size=ButtonSize::Sm variant=ButtonVariant::Outline on_click=move |_| store.upgraded.set(None)>"Got it"</Button></div>
            </div>
        })
    };

    let body = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        if data.flows.is_empty() && data.accounts.is_empty() {
            return view! {
                <EmptyState title="No data yet"
                    text="Start by adding your accounts and paychecks, then your bills and recurring expenses. Or load an example household to look around first (you can erase it later in Settings).">
                    <div class="flex gap-2">
                        <Button on_click=move |_| on_nav.run(Page::Accounts)>"Add accounts"</Button>
                        <Button variant=ButtonVariant::Outline on_click=move |_| on_nav.run(Page::Income)>"Add income"</Button>
                        <Button variant=ButtonVariant::Secondary on_click=move |_| store.call("reset_data", KindArg { kind: "sample" }, "Sample data loaded")>"Load sample data"</Button>
                    </div>
                </EmptyState>
            }
            .into_any();
        }
        let income = data.monthly_total(|k| k == FlowKind::Income, today);
        let bills = data.monthly_total(|k| k == FlowKind::Bill, today);
        let expenses = data.monthly_total(|k| k == FlowKind::Expense, today);
        let cash_contrib = data.monthly_cash_contributions();
        let payroll_contrib: f64 = data.accounts.iter().filter(|a| !a.contribution_from_cash).map(|a| a.monthly_contribution).sum();
        let match_total: f64 = data.accounts.iter().map(|a| a.employer_match_monthly).sum();
        let debt_plan = data.debt_monthly_budget();
        let debt_total = data.total_debt();
        let leftover = income - bills - expenses - cash_contrib - debt_plan;
        let saved = cash_contrib + leftover.max(0.0);
        let savings_rate = if income > 0.0 { saved / income * 100.0 } else { 0.0 };
        let years = data.settings.projection_years.max(1);
        let proj = projection.get();
        let yearly = proj.yearly();
        let labels: Vec<String> = yearly.iter().map(|y| y.year.to_string()).collect();
        let nw10 = proj.at_years(10.min(years)).map(|m| m.net_worth()).unwrap_or(0.0);
        let nw_end = proj.months.last().map(|m| m.net_worth()).unwrap_or(0.0);
        let mut series = vec![
            Series::new("Net worth", "var(--chart-3)", yearly.iter().map(|y| y.net_worth()).collect(), true),
            Series::new("Retirement", "var(--success)", yearly.iter().map(|y| y.retirement).collect(), false),
            Series::new("Investments", "var(--warning)", yearly.iter().map(|y| y.investments).collect(), false),
            Series::new("Cash", "var(--chart-1)", yearly.iter().map(|y| y.cash).collect(), false),
        ];
        if debt_total > 0.0 {
            series.push(Series::new("Debt", "var(--destructive)", yearly.iter().map(|y| y.debt).collect(), false));
        }
        // First projected month with nothing owed.
        let debt_free = (debt_total > 0.0).then(|| proj.months.iter().find(|m| m.debt <= 0.005).map(|m| m.month)).flatten();
        let shortfall = proj.cash_shortfall.map(|when| view! {
            <div class="flex items-start gap-3 rounded-xl border border-destructive/50 bg-destructive-light p-4 text-sm text-destructive-dark">
                <Icon name=IconName::Alert class="size-5 shrink-0" />
                <div>
                    <div class="font-semibold">{format!("Cash runs short in {}", when.format("%B %Y"))}</div>
                    <div>"At the current pace, your checking and savings balances would go below zero. Look at bills, expenses or contributions paid from cash."</div>
                </div>
            </div>
        });
        let net_sub = format!(
            "Cash {} · Invest {} · Retire {}{}",
            money_short(data.group_total(AccountGroup::Cash), &sym),
            money_short(data.group_total(AccountGroup::Investments), &sym),
            money_short(data.group_total(AccountGroup::Retirement), &sym),
            if debt_total > 0.0 { format!(" · Debt −{}", money_short(debt_total, &sym)) } else { String::new() }
        );
        let left_sub = if debt_plan > 0.0 {
            format!("After {} to savings and {} to debt", money(cash_contrib, &sym), money(debt_plan, &sym))
        } else {
            format!("After {} moved to savings/investing", money(cash_contrib, &sym))
        };
        let left_tone = if leftover >= 0.0 { "good" } else { "bad" };
        let today_dollars = format!("{} in today's dollars", money(nw_end / proj.months.last().map(|m| m.inflation_factor).unwrap_or(1.0), &sym));
        let chart_note = format!(
            "Year-end balances using your income, bills, expenses, contributions, debt payments and expected returns.{}",
            debt_free.map(|d| format!(" Debt-free in {}.", d.format("%B %Y"))).unwrap_or_default()
        );
        let debts_view = (!data.debts.is_empty()).then(|| view! { <DebtOverview debt_free=debt_free /> });

        let up = upcoming(&data, today, 30);
        let up_out: f64 = up.iter().filter(|u| u.kind.is_outflow()).map(|u| u.amount).sum();
        let up_in: f64 = up.iter().filter(|u| !u.kind.is_outflow()).map(|u| u.amount).sum();
        let more = (up.len() > 14).then(|| view! { <p class="pt-2 text-xs text-muted-foreground">{format!("+ {} more", up.len() - 14)}</p> });
        let nothing = up.is_empty().then(|| view! { <p class="text-sm text-muted-foreground">"Nothing scheduled."</p> });
        let rows = up.into_iter().take(14).map(|u| {
            let kind_badge = match u.kind {
                FlowKind::Income => view! { <Badge variant=BadgeVariant::Success>"income"</Badge> },
                FlowKind::Bill => view! { <Badge variant=BadgeVariant::Warning>"bill"</Badge> },
                FlowKind::Expense => view! { <Badge variant=BadgeVariant::Info>"expense"</Badge> },
            };
            let amount_class = if u.kind.is_outflow() { "w-24 text-right tabular-nums" } else { "w-24 text-right tabular-nums text-success" };
            let amount = format!("{}{}", if u.kind.is_outflow() { "-" } else { "+" }, money_cents(u.amount, &sym));
            view! {
                <div class="flex items-center gap-3 border-b py-2 text-sm last:border-0">
                    <div class="w-16 shrink-0 text-xs text-muted-foreground">{u.date.format("%a %b %-d").to_string()}</div>
                    <div class="flex-1 truncate">{u.name.clone()}</div>
                    {u.autopay.then(|| view! { <Badge variant=BadgeVariant::Muted>"autopay"</Badge> })}
                    {kind_badge}
                    <div class=amount_class>{amount}</div>
                </div>
            }
        }).collect_view();
        let cats = spending_by_category(&data, today);
        let cat_total: f64 = cats.iter().map(|c| c.1).sum::<f64>().max(1.0);
        let no_cats = cats.is_empty().then(|| view! { <p class="text-sm text-muted-foreground">"No bills or expenses yet."</p> });
        let cat_rows = cats.into_iter().map(|(cat, amt)| view! {
            <div class="flex flex-col gap-1">
                <div class="flex justify-between text-sm">
                    <span>{cat}</span>
                    <span class="tabular-nums text-muted-foreground">{money(amt, &sym)}</span>
                </div>
                <Progress value=amt max=cat_total />
            </div>
        }).collect_view();

        let next30 = format!("In {} · Out {}", money(up_in, &sym), money(up_out, &sym));
        let chart_sym = sym.clone();
        view! {
            {shortfall}
            <div class="grid grid-cols-2 gap-4 @3xl:grid-cols-4">
                <Stat label="Net worth" value=money(data.net_worth(), &sym) sub=net_sub />
                <Stat label="Monthly income" value=money(income, &sym) sub="Average take-home per month" />
                <Stat label="Bills + expenses" value=money(bills + expenses, &sym) sub=format!("Bills {} · Expenses {}", money(bills, &sym), money(expenses, &sym)) />
                <Stat label="Left over / month" value=money(leftover, &sym) tone=left_tone sub=left_sub />
                <Stat label="Savings rate" value=format!("{savings_rate:.0}%") sub="Contributions + leftover cash, as % of income" />
                <Stat label="Paycheck + employer" value=money(payroll_contrib + match_total, &sym) sub="Monthly into retirement from payroll & match" />
                <Stat label=format!("Net worth in {} yrs", 10.min(years)) value=money(nw10, &sym) sub="Projected, nominal dollars" />
                <Stat label=format!("Net worth in {years} yrs") value=money(nw_end, &sym) sub=today_dollars />
            </div>
            <Card>
                <CardHeader>
                    <CardTitle>"Projected net worth"</CardTitle>
                    <CardDescription>{chart_note}</CardDescription>
                </CardHeader>
                <CardContent><LineChart labels=labels series=series currency=chart_sym /></CardContent>
            </Card>
            {debts_view}
            <div class="grid gap-6 @3xl:grid-cols-5">
                <Card class="@3xl:col-span-3">
                    <CardHeader>
                        <CardTitle>"Next 30 days"</CardTitle>
                        <CardDescription>{next30}</CardDescription>
                    </CardHeader>
                    <CardContent class="flex flex-col">{nothing}{rows}{more}</CardContent>
                </Card>
                <Card class="@3xl:col-span-2">
                    <CardHeader>
                        <CardTitle>"Monthly spending by category"</CardTitle>
                        <CardDescription>"Bills and recurring expenses, monthly average"</CardDescription>
                    </CardHeader>
                    <CardContent class="flex flex-col gap-3">{no_cats}{cat_rows}</CardContent>
                </Card>
            </div>
        }
        .into_any()
    };

    view! {
        <PageHeader title="Dashboard" subtitle=format!("As of {}", today.format("%B %-d, %Y")) />
        {upgraded}
        {body}
    }
}

/// The debt plan in brief, with each balance over time.
#[component]
fn DebtOverview(debt_free: Option<NaiveDate>) -> impl IntoView {
    let store = use_store();
    let data = store.data_untracked();
    let sym = data.settings.currency_symbol.clone();
    let p = plans_for(&data, store.today()).chosen;
    let monthly = data.debt_monthly_budget();
    if p.months.is_empty() {
        return ().into_any();
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
        .filter(|(_, i)| **i < data.debts.len())
        .map(|(k, &i)| Series::new(data.debts[i].name.clone(), COLORS[k % COLORS.len()], idx.iter().map(|&m| p.months[m].balances[i]).collect(), false))
        .collect();
    let total = Series::new("Total owed", "var(--foreground)", idx.iter().map(|&m| p.months[m].balances.iter().sum()).collect(), true);
    let labels: Vec<String> = idx.iter().map(|&m| p.months[m].month.format("%b %y").to_string()).collect();
    let strategy = match data.settings.debt_strategy {
        Strategy::Avalanche => "avalanche",
        Strategy::Snowball => "snowball",
    };
    let when = debt_free.or(p.debt_free);
    let description = match when {
        Some(d) => format!("Paying {} a month ({strategy}), you're debt-free in {} with {} of interest.", money(monthly, &sym), d.format("%B %Y"), money(p.total_interest, &sym)),
        None => format!("At {} a month the debt isn't paid off within 50 years. See Debt payoff.", money(monthly, &sym)),
    };
    let figure = |label: &'static str, value: String, class: &'static str| view! {
        <div class="flex flex-col">
            <span class="text-xs text-muted-foreground">{label}</span>
            <span class=format!("text-lg font-semibold tabular-nums {class}")>{value}</span>
        </div>
    };
    view! {
        <Card>
            <CardHeader>
                <CardTitle>"Debt payoff"</CardTitle>
                <CardDescription>{description}</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-4">
                <div class="grid grid-cols-2 gap-4 @3xl:grid-cols-4">
                    {figure("Owed now", money(data.total_debt(), &sym), "text-destructive")}
                    {figure("Paid each month", money(monthly, &sym), "")}
                    {figure("Debt-free", when.map(|d| d.format("%b %Y").to_string()).unwrap_or_else(|| "—".into()), "text-success")}
                    {figure("Interest to pay", money(p.total_interest, &sym), "")}
                </div>
                <StackedChart layers=layers total=total labels=labels currency=sym.clone() />
            </CardContent>
        </Card>
    }
    .into_any()
}
