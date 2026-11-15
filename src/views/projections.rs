use crate::app::{today, use_store};
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::switch::Switch;
use crate::ui::table::*;
use crate::views::dashboard::PageHeader;
use crate::widgets::*;
use dioxus::prelude::*;
use chrono::NaiveDate;
use penguinfin::model::{Account, AccountGroup, AccountKind, AppData, CashFlow, FlowKind, Frequency};
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

    let years = parse_years(&years_txt.read(), data.settings.projection_years);
    let what_if = read_what_if(extra_income, extra_spend, extra_invest, return_shift);

    // Both projections run on a worker thread; skeletons show until they're ready.
    // Re-runs whenever the data, the year count or a what-if field changes.
    let projections = use_resource(move || {
        let data = store.data.read().clone();
        let years = parse_years(&years_txt.read(), data.settings.projection_years);
        let wi = read_what_if(extra_income, extra_spend, extra_invest, return_shift);
        async move {
            crate::app::off_thread(move || {
                let today = today();
                let base = project(&data, today, years);
                let scenario = wi.is_active().then(|| project(&build_scenario(&data, &wi, today), today, years));
                (base, scenario)
            })
            .await
        }
    });
    let result = projections.read().clone();
    let loading = result.is_none();
    let (base, scenario) = result.unwrap_or_default();
    let showing_what_if = scenario.is_some();
    let proj = scenario.unwrap_or_else(|| base.clone());
    let yearly = proj.yearly();
    let base_yearly = base.yearly();
    let what_if = what_if.is_active() && showing_what_if;
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

    let inflation_source_note = match data.fred.inflation_source.indicator() {
        Some(i) => format!(", {} from FRED", i.label().to_lowercase()),
        None => String::new(),
    };
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
                CardDescription {
                    if real() { "In today's dollars (adjusted for {num_str(data.settings.inflation_pct)}% inflation{inflation_source_note})" }
                    else { "Nominal (future) dollars" }
                }
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
                if loading {
                    ChartSkeleton {}
                } else {
                    LineChart { labels, series, currency: sym.clone() }
                }
            }
        }

        Card {
            CardHeader {
                CardTitle { "What if…" }
                CardDescription { "Try changes without saving them. Amounts are per month." }
            }
            CardContent { class: "grid grid-cols-2 gap-4 @3xl:grid-cols-4",
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

        if !data.market.values.is_empty() {
            Card { size: CardSize::Sm,
                CardHeader {
                    CardTitle { "Market context" }
                    CardDescription { "Latest figures from FRED (synced {data.market.last_sync.clone().unwrap_or_default()}). Compare them with your return and inflation assumptions." }
                }
                CardContent { class: "grid grid-cols-2 gap-x-6 gap-y-3 @3xl:grid-cols-3",
                    for v in data.market.values.iter() {
                        div { key: "{v.indicator.key()}", class: "flex flex-col",
                            span { class: "text-xs text-muted-foreground", "{v.indicator.label()}" }
                            span { class: "font-semibold tabular-nums", "{v.value:.2}%" }
                        }
                    }
                }
            }
        }

        div { class: "grid grid-cols-2 gap-4 @3xl:grid-cols-4",
            if loading {
                for m in milestones.iter().copied() {
                    Stat { key: "{m}", label: format!("In {m} year{}", if m == 1 { "" } else { "s" }), value: "", loading: true }
                }
            }
            for m in milestones.iter().copied() {
                if let Some(pt) = proj.at_years(m).filter(|_| !loading) {
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
                if loading {
                    TableSkeleton { rows: 8 }
                } else {
                TableWrapper { class: "max-h-[30rem]",
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
                }
                p { class: "mt-3 text-xs text-muted-foreground",
                    "Projections are estimates, not advice. They ignore taxes on withdrawals, market swings and fees unless you include them in your return assumptions." }
            }
        }
    }
}

fn parse_years(text: &str, default: u32) -> u32 {
    text.trim().parse::<u32>().ok().filter(|y| (1..=80).contains(y)).unwrap_or(default)
}

/// Unsaved what-if adjustments, per month.
#[derive(Clone, Copy, PartialEq, Default)]
struct WhatIf {
    income: f64,
    spend: f64,
    invest: f64,
    return_shift: f64,
}

impl WhatIf {
    fn is_active(&self) -> bool {
        self.income != 0.0 || self.spend != 0.0 || self.invest != 0.0 || self.return_shift != 0.0
    }
}

fn read_what_if(income: Signal<String>, spend: Signal<String>, invest: Signal<String>, shift: Signal<String>) -> WhatIf {
    let n = |s: Signal<String>| parse_num(&s.read()).unwrap_or(0.0);
    WhatIf { income: n(income), spend: n(spend), invest: n(invest), return_shift: n(shift) }
}

/// A copy of the data with the what-if adjustments applied.
fn build_scenario(data: &AppData, wi: &WhatIf, today: NaiveDate) -> AppData {
    let mut scenario = data.clone();
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
    if wi.income != 0.0 {
        scenario.flows.push(mk(FlowKind::Income, wi.income));
    }
    if wi.spend != 0.0 {
        scenario.flows.push(mk(FlowKind::Expense, wi.spend));
    }
    if wi.invest != 0.0 {
        scenario.accounts.push(Account {
            id: -1,
            name: "What-if investing".into(),
            kind: AccountKind::Brokerage,
            balance: 0.0,
            annual_return_pct: 7.0,
            monthly_contribution: wi.invest,
            contribution_from_cash: true,
            employer_match_monthly: 0.0,
            notes: String::new(),
        });
    }
    if wi.return_shift != 0.0 {
        for a in scenario.accounts.iter_mut().filter(|a| a.kind.group() != AccountGroup::Cash) {
            a.annual_return_pct += wi.return_shift;
        }
    }
    scenario
}
