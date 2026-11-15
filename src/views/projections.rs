use crate::app::{today, use_store};
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::switch::Switch;
use crate::ui::table::*;
use crate::views::dashboard::PageHeader;
use crate::widgets::*;
use dioxus::prelude::*;
use penguinfin::model::{Account, AccountKind, CashFlow, FlowKind, Frequency};
use penguinfin::projection::project;

#[component]
pub fn ProjectionsPage() -> Element {
    let mut store = use_store();
    let data = store.data.read().clone();
    let sym = data.settings.currency_symbol.clone();
    let mut real = use_signal(|| false);
    let mut years_txt = use_signal(|| data.settings.projection_years.to_string());
    // What-if adjustments (not saved).
    let mut extra_income = use_signal(String::new);
    let mut extra_spend = use_signal(String::new);
    let mut extra_invest = use_signal(String::new);
    let mut return_shift = use_signal(String::new);

    let today = today();
    let years = years_txt.read().trim().parse::<u32>().ok().filter(|y| (1..=80).contains(y)).unwrap_or(data.settings.projection_years);

    // Build the what-if scenario on a copy of the data.
    let mut scenario = data.clone();
    let wi_income = parse_num(&extra_income.read()).unwrap_or(0.0);
    let wi_spend = parse_num(&extra_spend.read()).unwrap_or(0.0);
    let wi_invest = parse_num(&extra_invest.read()).unwrap_or(0.0);
    let wi_ret = parse_num(&return_shift.read()).unwrap_or(0.0);
    let what_if = wi_income != 0.0 || wi_spend != 0.0 || wi_invest != 0.0 || wi_ret != 0.0;
    let mk = |kind, amount: f64| CashFlow {
        id: -1,
        kind,
        name: "What-if".into(),
        category: "What-if".into(),
        amount,
        frequency: Frequency::Monthly,
        start_date: today,
        end_date: None,
        annual_change_pct: 0.0,
        autopay: false,
        notes: String::new(),
    };
    if wi_income != 0.0 {
        scenario.flows.push(mk(FlowKind::Income, wi_income));
    }
    if wi_spend != 0.0 {
        scenario.flows.push(mk(FlowKind::Expense, wi_spend));
    }
    if wi_invest != 0.0 {
        scenario.accounts.push(Account {
            id: -1,
            name: "What-if investing".into(),
            kind: AccountKind::Brokerage,
            balance: 0.0,
            annual_return_pct: 7.0,
            monthly_contribution: wi_invest,
            contribution_from_cash: true,
            employer_match_monthly: 0.0,
            notes: String::new(),
        });
    }
    if wi_ret != 0.0 {
        for a in scenario.accounts.iter_mut().filter(|a| a.kind.group() != penguinfin::model::AccountGroup::Cash) {
            a.annual_return_pct += wi_ret;
        }
    }

    let base = project(&data, today, years);
    let proj = if what_if { project(&scenario, today, years) } else { base.clone() };
    let yearly = proj.yearly();
    let base_yearly = base.yearly();
    let adj = |v: f64, f: f64| if real() { v / f } else { v };
    let labels: Vec<String> = yearly.iter().map(|y| y.year.to_string()).collect();

    let mut series = vec![
        Series { name: if what_if { "Net worth (what-if)".into() } else { "Net worth".into() }, color: "var(--chart-3)".into(),
            values: yearly.iter().map(|y| adj(y.net_worth(), y.inflation_factor)).collect(), bold: true },
        Series { name: "Retirement".into(), color: "var(--success)".into(),
            values: yearly.iter().map(|y| adj(y.retirement, y.inflation_factor)).collect(), bold: false },
        Series { name: "Investments".into(), color: "var(--warning)".into(),
            values: yearly.iter().map(|y| adj(y.investments, y.inflation_factor)).collect(), bold: false },
        Series { name: "Cash".into(), color: "var(--chart-1)".into(),
            values: yearly.iter().map(|y| adj(y.cash, y.inflation_factor)).collect(), bold: false },
    ];
    if what_if {
        series.push(Series { name: "Net worth (current plan)".into(), color: "var(--muted-foreground)".into(),
            values: base_yearly.iter().map(|y| adj(y.net_worth(), y.inflation_factor)).collect(), bold: false });
    }

    let milestones: Vec<u32> = [1u32, 5, 10, 15, 20, 25, 30, 40].into_iter().filter(|m| *m <= years).collect();

    rsx! {
        PageHeader { title: "Projections",
            subtitle: "How your balances grow from your income, bills, expenses, contributions and expected returns." }

        if let Some(when) = proj.cash_shortfall {
            div { class: "flex items-start gap-3 rounded-xl border border-destructive/50 bg-destructive-light p-4 text-sm text-destructive-dark",
                Icon { name: IconName::Alert, class: "size-5 shrink-0" }
                "Cash goes negative in {when.format(\"%B %Y\")}. After that point the projection assumes you are borrowing (negative cash)."
            }
        }

        Card {
            CardHeader {
                CardTitle { "Net worth over time" }
                CardDescription { if real() { "In today's dollars (adjusted for {num_str(data.settings.inflation_pct)}% inflation)" } else { "Nominal (future) dollars" } }
            }
            CardContent { class: "flex flex-col gap-4",
                div { class: "flex flex-wrap items-end gap-6",
                    Field { label: "Years to project",
                        div { class: "flex items-center gap-2",
                            Input { class: "w-24", r#type: InputType::Number, value: years_txt(),
                                oninput: move |e: FormEvent| years_txt.set(e.value()) }
                            crate::ui::button::Button {
                                variant: crate::ui::button::ButtonVariant::Ghost, size: crate::ui::button::ButtonSize::Sm,
                                onclick: move |_| {
                                    let mut s = store.data.read().settings.clone();
                                    s.projection_years = years;
                                    store.mutate("Saved as default", |db| db.save_settings(&s));
                                },
                                "Make default"
                            }
                        }
                    }
                    div { class: "flex items-center gap-2 pb-1.5",
                        Switch { checked: real(), aria_label: "Inflation adjusted", onchange: move |v| real.set(v) }
                        span { class: "text-sm", "Show in today's dollars" }
                    }
                }
                LineChart { labels, series, currency: sym.clone() }
            }
        }

        Card {
            CardHeader {
                CardTitle { "What if…" }
                CardDescription { "Try changes without saving them. Amounts are per month." }
            }
            CardContent { class: "grid grid-cols-2 gap-4 lg:grid-cols-4",
                Field { label: "Extra income / month",
                    Input { placeholder: "0", value: extra_income(), oninput: move |e: FormEvent| extra_income.set(e.value()) } }
                Field { label: "Extra spending / month",
                    Input { placeholder: "0", value: extra_spend(), oninput: move |e: FormEvent| extra_spend.set(e.value()) } }
                Field { label: "Invest extra / month", hint: "Into a brokerage at 7%",
                    Input { placeholder: "0", value: extra_invest(), oninput: move |e: FormEvent| extra_invest.set(e.value()) } }
                Field { label: "Change all returns by %", hint: "e.g. -2 for a cautious case",
                    Input { placeholder: "0", value: return_shift(), oninput: move |e: FormEvent| return_shift.set(e.value()) } }
            }
        }

        div { class: "grid grid-cols-2 gap-4 lg:grid-cols-4",
            for m in milestones.iter().copied() {
                if let Some(pt) = proj.at_years(m) {
                    Stat {
                        key: "{m}",
                        label: format!("In {m} year{}", if m == 1 { "" } else { "s" }),
                        value: money(adj(pt.net_worth(), pt.inflation_factor), &sym),
                        sub: format!("Retirement {} · Invest {}",
                            money_short(adj(pt.retirement, pt.inflation_factor), &sym),
                            money_short(adj(pt.investments, pt.inflation_factor), &sym)),
                        tone: if pt.net_worth() < 0.0 { "bad" } else { "" },
                    }
                }
            }
        }

        Card {
            CardHeader { CardTitle { "Year by year" } CardDescription { "Totals for each calendar year; balances at year end." } }
            CardContent {
                TableWrapper { class: "max-h-[480px]",
                    Table {
                        TableHeader {
                            TableRow {
                                TableHead { "Year" }
                                TableHead { class: "text-right", "Income" }
                                TableHead { class: "text-right", "Bills + expenses" }
                                TableHead { class: "text-right", "Saved from cash" }
                                TableHead { class: "text-right", "Cash" }
                                TableHead { class: "text-right", "Investments" }
                                TableHead { class: "text-right", "Retirement" }
                                TableHead { class: "text-right", "Net worth" }
                            }
                        }
                        TableBody {
                            for y in yearly.iter() {
                                TableRow { key: "{y.year}",
                                    TableCell { class: "font-medium", "{y.year}" }
                                    TableCell { class: "text-right tabular-nums", {money(adj(y.income, y.inflation_factor), &sym)} }
                                    TableCell { class: "text-right tabular-nums", {money(adj(y.outflows, y.inflation_factor), &sym)} }
                                    TableCell { class: "text-right tabular-nums", {money(adj(y.contributions, y.inflation_factor), &sym)} }
                                    TableCell { class: if y.cash < 0.0 { "text-right tabular-nums text-destructive" } else { "text-right tabular-nums" },
                                        {money(adj(y.cash, y.inflation_factor), &sym)} }
                                    TableCell { class: "text-right tabular-nums", {money(adj(y.investments, y.inflation_factor), &sym)} }
                                    TableCell { class: "text-right tabular-nums", {money(adj(y.retirement, y.inflation_factor), &sym)} }
                                    TableCell { class: "text-right font-medium tabular-nums", {money(adj(y.net_worth(), y.inflation_factor), &sym)} }
                                }
                            }
                        }
                    }
                }
                p { class: "mt-3 text-xs text-muted-foreground",
                    "Projections are estimates, not advice. They ignore taxes on withdrawals, market swings and fees unless you include them in your return assumptions." }
            }
        }
    }
}
