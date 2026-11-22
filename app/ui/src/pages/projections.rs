//! Projections: net worth over time, an account split into buckets, what-if changes, market
//! context and the year-by-year table. The projection engine runs here in the interface.

use crate::app::use_store;
use crate::charts::{LineChart, Series, StackedChart};
use crate::ui::icon::{Icon, IconName};
use crate::ui::*;
use chrono::NaiveDate;
use leptos::prelude::*;
use penguinfin_core::format::{money, money_short, num_str, parse_num};
use penguinfin_core::ipc::Change;
use penguinfin_core::model::{Account, AccountGroup, AccountKind, AppData, Bucket, CashFlow, FlowKind, Frequency};
use penguinfin_core::projection::{Projection, project};

#[component]
pub fn ProjectionsPage() -> impl IntoView {
    let store = use_store();
    let today = store.today();
    let default_years = store.snap.with_untracked(|s| s.as_ref().map(|s| s.data.settings.projection_years).unwrap_or(30));
    let real = RwSignal::new(false);
    let years_txt = RwSignal::new(default_years.to_string());
    // What-if adjustments (not saved).
    let extra_income = RwSignal::new(String::new());
    let extra_spend = RwSignal::new(String::new());
    let extra_invest = RwSignal::new(String::new());
    let return_shift = RwSignal::new(String::new());
    let years = Memo::new(move |_| parse_years(&years_txt.get(), default_years));
    let what_if = Memo::new(move |_| read_what_if(extra_income, extra_spend, extra_invest, return_shift));
    // (current plan, what-if) — re-run when the data, the years or a what-if field changes.
    let projections = Memo::new(move |_| {
        let data = store.data();
        let y = years.get();
        let wi = what_if.get();
        let base = project(&data, today, y);
        let scenario = wi.is_active().then(|| project(&build_scenario(&data, &wi, today), today, y));
        (base, scenario)
    });

    let chart = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        let (base, scenario) = projections.get();
        let showing_what_if = scenario.is_some();
        let proj = scenario.unwrap_or_else(|| base.clone());
        let yearly = proj.yearly();
        let base_yearly = base.yearly();
        let r = real.get();
        let adj = move |v: f64, f: f64| if r { v / f } else { v };
        let labels: Vec<String> = yearly.iter().map(|y| y.year.to_string()).collect();
        let mut series = vec![
            Series::new(if showing_what_if { "Net worth (what-if)" } else { "Net worth" }, "var(--chart-3)",
                yearly.iter().map(|y| adj(y.net_worth(), y.inflation_factor)).collect(), true),
            Series::new("Retirement", "var(--success)", yearly.iter().map(|y| adj(y.retirement, y.inflation_factor)).collect(), false),
            Series::new("Investments", "var(--warning)", yearly.iter().map(|y| adj(y.investments, y.inflation_factor)).collect(), false),
            Series::new("Cash", "var(--chart-1)", yearly.iter().map(|y| adj(y.cash, y.inflation_factor)).collect(), false),
        ];
        if !data.debts.is_empty() {
            series.push(Series::new("Debt", "var(--destructive)", yearly.iter().map(|y| adj(y.debt, y.inflation_factor)).collect(), false));
        }
        if showing_what_if {
            series.push(Series::new("Net worth (current plan)", "var(--muted-foreground)",
                base_yearly.iter().map(|y| adj(y.net_worth(), y.inflation_factor)).collect(), false));
        }
        view! { <LineChart labels=labels series=series currency=sym /> }
    };
    let description = move || {
        let data = store.data();
        if real.get() {
            let note = match data.fred.inflation_source.indicator() {
                Some(i) => format!(", {} from FRED", i.label().to_lowercase()),
                None => String::new(),
            };
            format!("In today's dollars (adjusted for {}% inflation{note})", num_str(data.settings.inflation_pct))
        } else {
            "Nominal (future) dollars".to_string()
        }
    };
    let shortfall = move || {
        projections.with(|(b, s)| s.as_ref().unwrap_or(b).cash_shortfall).map(|when| view! {
            <div class="flex items-start gap-3 rounded-xl border border-destructive/50 bg-destructive-light p-4 text-sm text-destructive-dark">
                <Icon name=IconName::Alert class="size-5 shrink-0" />
                {format!("Cash goes negative in {}. After that point the projection assumes you are borrowing (negative cash).", when.format("%B %Y"))}
            </div>
        })
    };
    let market = move || {
        let snap = store.snap.get().unwrap_or_default();
        (!snap.market.values.is_empty()).then(|| view! {
            <Card small=true>
                <CardHeader>
                    <CardTitle>"Market context"</CardTitle>
                    <CardDescription>{format!("Latest figures from FRED (synced {}). Compare them with your return and inflation assumptions.", snap.market.last_sync.clone().unwrap_or_default())}</CardDescription>
                </CardHeader>
                <CardContent class="grid grid-cols-2 gap-x-6 gap-y-3 @3xl:grid-cols-3">
                    {snap.market.values.iter().map(|v| view! {
                        <div class="flex flex-col">
                            <span class="text-xs text-muted-foreground">{v.indicator.label()}</span>
                            <span class="font-semibold tabular-nums">{format!("{:.2}%", v.value)}</span>
                        </div>
                    }).collect_view()}
                </CardContent>
            </Card>
        })
    };
    let milestones = move || {
        let sym = store.currency();
        let (base, scenario) = projections.get();
        let proj = scenario.unwrap_or(base);
        let r = real.get();
        let adj = move |v: f64, f: f64| if r { v / f } else { v };
        [1u32, 5, 10, 15, 20, 25, 30, 40]
            .into_iter()
            .filter(|m| *m <= years.get())
            .filter_map(|m| {
                let pt = proj.at_years(m)?;
                Some(view! {
                    <Stat label=format!("In {m} year{}", if m == 1 { "" } else { "s" }) value=money(adj(pt.net_worth(), pt.inflation_factor), &sym)
                        sub=format!("Retirement {} · Invest {}", money_short(adj(pt.retirement, pt.inflation_factor), &sym), money_short(adj(pt.investments, pt.inflation_factor), &sym))
                        tone=if pt.net_worth() < 0.0 { "bad" } else { "" } />
                })
            })
            .collect_view()
    };
    let table = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        let has_debt = !data.debts.is_empty();
        let (base, scenario) = projections.get();
        let r = real.get();
        let adj = move |v: f64, f: f64| if r { v / f } else { v };
        let rows = scenario.unwrap_or(base).yearly().into_iter().map(|y| {
            let f = y.inflation_factor;
            let c = |v: f64| money(adj(v, f), &sym);
            let (year, income, outflows, contributions, cash, investments, retirement, debt_v, net) = (
                y.year.to_string(), c(y.income), c(y.outflows), c(y.contributions), c(y.cash), c(y.investments), c(y.retirement), c(y.debt), c(y.net_worth()),
            );
            let cash_class = if y.cash < 0.0 { "text-right tabular-nums text-destructive" } else { "text-right tabular-nums" };
            let debt = has_debt.then(move || view! { <TableCell class="text-right tabular-nums text-destructive">{debt_v}</TableCell> });
            view! {
                <TableRow>
                    <TableCell class="font-medium">{year}</TableCell>
                    <TableCell class="text-right tabular-nums">{income}</TableCell>
                    <TableCell class="text-right tabular-nums">{outflows}</TableCell>
                    <TableCell class="text-right tabular-nums">{contributions}</TableCell>
                    <TableCell class=cash_class>{cash}</TableCell>
                    <TableCell class="text-right tabular-nums">{investments}</TableCell>
                    <TableCell class="text-right tabular-nums">{retirement}</TableCell>
                    {debt}
                    <TableCell class="text-right font-medium tabular-nums">{net}</TableCell>
                </TableRow>
            }
        }).collect_view();
        view! {
            <TableWrapper class="max-h-[30rem]">
                <Table>
                    <TableHeader>
                        <TableRow>
                            <TableHead>"Year"</TableHead>
                            <TableHead class="text-right">"Income"</TableHead>
                            <TableHead class="text-right">"Bills + expenses"</TableHead>
                            <TableHead class="text-right">"Saved from cash"</TableHead>
                            <TableHead class="text-right">"Cash"</TableHead>
                            <TableHead class="text-right">"Investments"</TableHead>
                            <TableHead class="text-right">"Retirement"</TableHead>
                            {has_debt.then(|| view! { <TableHead class="text-right">"Debt"</TableHead> })}
                            <TableHead class="text-right">"Net worth"</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>{rows}</TableBody>
                </Table>
            </TableWrapper>
        }
    };
    let make_default = move |_| {
        let mut s = store.snap.with_untracked(|s| s.as_ref().map(|s| s.data.settings.clone()).unwrap_or_default());
        s.projection_years = years.get_untracked();
        store.apply(Change::SaveSettings(s), "Saved as default");
    };

    view! {
        <PageHeader title="Projections" subtitle="How your balances grow from your income, bills, expenses, contributions and expected returns." />
        {shortfall}
        <Card>
            <CardHeader>
                <CardTitle>"Net worth over time"</CardTitle>
                <CardDescription>{description}</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-4">
                <div class="flex flex-wrap items-end gap-6">
                    <Field label="Years to project">
                        <div class="flex items-center gap-2">
                            <div class="w-24"><Input value=years_txt kind="number" /></div>
                            <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm on_click=make_default>"Make default"</Button>
                        </div>
                    </Field>
                    <div class="flex items-center gap-2 pb-1.5">
                        <Switch checked=real label="Inflation adjusted" on_change=move |v| real.set(v) />
                        <span class="text-sm">"Show in today's dollars"</span>
                    </div>
                </div>
                {chart}
            </CardContent>
        </Card>
        {move || store.snap.with(|s| s.as_ref().is_some_and(|s| !s.data.accounts.is_empty())).then(|| view! { <AccountsBucketsCard projections=projections /> })}
        <Card>
            <CardHeader>
                <CardTitle>"What if…"</CardTitle>
                <CardDescription>"Try changes without saving them. Amounts are per month."</CardDescription>
            </CardHeader>
            <CardContent class="grid grid-cols-2 gap-4 @3xl:grid-cols-4">
                <Field label="Extra income / month"><Input value=extra_income placeholder="0" /></Field>
                <Field label="Extra spending / month"><Input value=extra_spend placeholder="0" /></Field>
                <Field label="Invest extra / month" hint="Into a brokerage at 7%"><Input value=extra_invest placeholder="0" /></Field>
                <Field label="Change all returns by %" hint="e.g. -2 for a cautious case"><Input value=return_shift placeholder="0" /></Field>
            </CardContent>
        </Card>
        {market}
        <div class="grid grid-cols-2 gap-4 @3xl:grid-cols-4">{milestones}</div>
        <Card>
            <CardHeader>
                <CardTitle>"Year by year"</CardTitle>
                <CardDescription>"Totals for each calendar year; balances at year end."</CardDescription>
            </CardHeader>
            <CardContent>
                {table}
                <p class="mt-3 text-xs text-muted-foreground">
                    "Projections are estimates, not advice. They ignore taxes on withdrawals, market swings and fees unless you include them in your return assumptions."
                </p>
            </CardContent>
        </Card>
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

fn read_what_if(income: RwSignal<String>, spend: RwSignal<String>, invest: RwSignal<String>, shift: RwSignal<String>) -> WhatIf {
    let n = |s: RwSignal<String>| parse_num(&s.get()).unwrap_or(0.0);
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
        account_id: None,
        bucket_id: None,
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
            purpose: String::new(),
            minimum_balance: 0.0,
        });
    }
    if wi.return_shift != 0.0 {
        for a in scenario.accounts.iter_mut().filter(|a| a.kind.group() != AccountGroup::Cash) {
            a.annual_return_pct += wi.return_shift;
        }
    }
    scenario
}

const LAYER_COLORS: [&str; 7] = [
    "var(--chart-3)",
    "var(--success)",
    "var(--warning)",
    "var(--info)",
    "var(--chart-5)",
    "var(--destructive)",
    "var(--chart-2)",
];


/// One account over time, split into its buckets and unallocated money.
#[component]
fn AccountsBucketsCard(projections: Memo<(Projection, Option<Projection>)>) -> impl IntoView {
    let store = use_store();
    let default_id = move || {
        let data = store.data();
        data.accounts
            .iter()
            .find(|a| data.buckets_for(a.id).next().is_some())
            .or(data.primary_cash_account())
            .or(data.accounts.first())
            .map(|a| a.id)
    };
    let selected = RwSignal::new(None::<i64>);
    // Months shown: 24, 60, or 0 = whole projection (one point per year).
    let range = RwSignal::new(24usize);
    let account_id = Memo::new(move |_| {
        let data = store.data();
        selected.get().filter(|id| data.accounts.iter().any(|a| a.id == *id)).or_else(default_id)
    });
    let options = move || store.data().accounts.iter().map(|a| (a.id.to_string(), format!("{} ({})", a.name, a.kind.label()))).collect::<Vec<_>>();

    let body = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        let Some(account) = account_id.get().and_then(|id| data.accounts.iter().find(|a| a.id == id)).cloned() else {
            return ().into_any();
        };
        let proj = projections.with(|(b, _)| b.clone());
        let Some(ai) = proj.account_ids.iter().position(|id| *id == account.id) else { return ().into_any() };
        if proj.months.is_empty() {
            return ().into_any();
        }
        let flow = data.money_flow(account.id, store.today());
        let bucket_pos: Vec<(usize, Bucket)> = proj
            .bucket_ids
            .iter()
            .enumerate()
            .filter_map(|(i, id)| data.bucket(*id).filter(|b| b.account_id == account.id).map(|b| (i, b.clone())))
            .collect();
        let r = range.get();
        let idx: Vec<usize> = match r {
            0 => (11..proj.months.len()).step_by(12).collect(),
            m => (0..proj.months.len().min(m)).collect(),
        };
        let labels: Vec<String> = idx
            .iter()
            .map(|&i| {
                let d = proj.months[i].month;
                if r == 0 { d.format("%Y").to_string() } else { d.format("%b %y").to_string() }
            })
            .collect();
        let bucket_sum = |i: usize| bucket_pos.iter().map(|(p, _)| proj.months[i].buckets[*p]).sum::<f64>();
        let mut layers = vec![Series::new("Unallocated", "var(--chart-muted)", idx.iter().map(|&i| (proj.months[i].accounts[ai] - bucket_sum(i)).max(0.0)).collect(), false)];
        for (n, (p, b)) in bucket_pos.iter().enumerate() {
            layers.push(Series::new(b.name.clone(), LAYER_COLORS[n % LAYER_COLORS.len()], idx.iter().map(|&i| proj.months[i].buckets[*p]).collect(), false));
        }
        let total = Series::new("Balance", "var(--foreground)", idx.iter().map(|&i| proj.months[i].accounts[ai]).collect(), true);
        let reference = (account.minimum_balance > 0.0).then(|| ("Minimum".to_string(), account.minimum_balance));

        let mut warnings: Vec<String> = Vec::new();
        for (id, when) in &proj.bucket_dry {
            if let Some((_, b)) = bucket_pos.iter().find(|(_, b)| b.id == *id) {
                warnings.push(format!("{} runs dry in {}. Bills paid from it then come out of {}'s unallocated money.", b.name, when.format("%B %Y"), account.name));
            }
        }
        if account.minimum_balance > 0.0
            && let Some(m) = proj.months.iter().find(|m| m.accounts[ai] < account.minimum_balance)
        {
            warnings.push(format!("{} drops below its {} minimum in {}.", account.name, money(account.minimum_balance, &sym), m.month.format("%B %Y")));
        }
        if !bucket_pos.is_empty()
            && let Some((i, _)) = proj.months.iter().enumerate().find(|(i, m)| bucket_sum(*i) > m.accounts[ai] + 0.5)
        {
            warnings.push(format!("Buckets add up to more than {}'s balance in {}.", account.name, proj.months[i].month.format("%B %Y")));
        }
        let mut notes = Vec::new();
        if !flow.received.is_empty() {
            notes.push(format!("Receives {}/mo: {}", money(flow.received_monthly, &sym), flow.received.join(", ")));
        }
        if !flow.paid.is_empty() {
            notes.push(format!("Pays {}/mo: {}", money(flow.paid_monthly, &sym), flow.paid.join(", ")));
        }
        if notes.is_empty() {
            notes.push("No income or bills use this account yet. Pick it under \u{201c}Paid from\u{201d} / \u{201c}Deposited to\u{201d} on a bill, expense or income.".into());
        }
        let chart = match reference {
            Some(r) => view! { <StackedChart layers=layers total=total reference=r labels=labels currency=sym.clone() /> }.into_any(),
            None => view! { <StackedChart layers=layers total=total labels=labels currency=sym.clone() /> }.into_any(),
        };
        view! {
            {chart}
            <div class="flex flex-wrap gap-x-6 gap-y-1 text-xs text-muted-foreground">
                {notes.into_iter().map(|n| view! { <span>{n}</span> }).collect_view()}
            </div>
            {warnings.into_iter().map(|w| view! {
                <div class="flex items-start gap-2 rounded-lg border border-warning/50 bg-warning-light px-3 py-2 text-sm text-warning-dark">
                    <Icon name=IconName::Alert class="size-4 shrink-0 mt-0.5" />
                    {w}
                </div>
            }).collect_view()}
        }
        .into_any()
    };

    view! {
        <Card>
            <CardHeader>
                <CardTitle>"Accounts & buckets"</CardTitle>
                <CardDescription>"How one account's money is split over time: each bucket, what's unallocated, and its minimum balance. Bills and income use the account or bucket you picked for them."</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-4">
                <div class="flex flex-wrap items-end gap-4">
                    <div class="w-72 max-w-full">
                        <Field label="Account">
                            {move || view! {
                                <NativeSelect label="Account" value=Signal::derive(move || account_id.get().map(|i| i.to_string()).unwrap_or_default())
                                    options=options() on_change=move |v: String| selected.set(v.parse().ok()) />
                            }}
                        </Field>
                    </div>
                    <div class="inline-flex rounded-lg bg-muted p-1" role="radiogroup" aria-label="Range">
                        {[(24usize, "2 years"), (60, "5 years"), (0, "All")].into_iter().map(|(months, label)| view! {
                            <button type="button" role="radio" aria-checked=move || (range.get() == months).to_string()
                                class=move || if range.get() == months { "rounded-md bg-background px-3 py-1 text-sm font-medium shadow-sm" } else { "rounded-md px-3 py-1 text-sm text-muted-foreground hover:text-foreground cursor-pointer" }
                                on:click=move |_| range.set(months)>{label}</button>
                        }).collect_view()}
                    </div>
                </div>
                {body}
            </CardContent>
        </Card>
    }
}
