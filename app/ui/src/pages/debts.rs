//! Debt payoff: the monthly budget after bills, avalanche or snowball, the payoff plan, each
//! debt with its one-time payments, the schedule and payment history. Plans run here in the
//! interface with the engine's simulator.

use crate::app::use_store;
use crate::charts::{Series, StackedChart};
use crate::pages::flows::parse_date;
use crate::ui::icon::{Icon, IconName};
use crate::ui::*;
use chrono::NaiveDate;
use leptos::prelude::*;
use penguinfin_core::debt::{Debt, DebtKind, DebtPayment, PayoffPlan, Strategy, payment_for_months, plan, plan_with, planned_lumps};
use penguinfin_core::format::{money, money_cents, num_str, parse_num};
use penguinfin_core::ipc::Change;
use penguinfin_core::model::{AppData, FlowKind};

/// Plans run up to 50 years; anything longer is reported as "not paid off".
const MAX_MONTHS: u32 = 600;

pub fn months_text(n: usize) -> String {
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

fn note_suffix(notes: &str) -> String {
    if notes.is_empty() { String::new() } else { format!(" \u{b7} {notes}") }
}

#[derive(Clone, PartialEq)]
pub struct Plans {
    pub chosen: PayoffPlan,
    pub minimums_only: PayoffPlan,
}

/// The chosen plan and the minimums-only one, with planned one-time payments in both.
pub fn plans_for(data: &AppData, today: NaiveDate) -> Plans {
    let s = &data.settings;
    let lumps = planned_lumps(&data.debts, &data.debt_payments, today);
    Plans {
        chosen: plan_with(&data.debts, s.debt_strategy, s.debt_extra_monthly, true, today, MAX_MONTHS, &lumps),
        minimums_only: plan_with(&data.debts, s.debt_strategy, 0.0, false, today, MAX_MONTHS, &lumps),
    }
}

/// Debt indices in plan order.
fn plan_order(debts: &[Debt], plans: &Plans) -> Vec<usize> {
    if plans.chosen.order.len() == debts.len() { plans.chosen.order.clone() } else { (0..debts.len()).collect() }
}

fn save_settings(store: crate::app::Store, f: impl FnOnce(&mut penguinfin_core::model::Settings)) {
    let mut s = store.snap.with_untracked(|s| s.as_ref().map(|s| s.data.settings.clone()).unwrap_or_default());
    f(&mut s);
    store.apply(Change::SaveSettings(s), "");
}

#[component]
pub fn DebtsPage() -> impl IntoView {
    let store = use_store();
    let today = store.today();
    let form = RwSignal::new(DebtForm::new());
    let plans = Memo::new(move |_| plans_for(&store.data(), today));

    let stats = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        let p = plans.get();
        let active = data.debts.iter().filter(|d| d.balance > 0.0).count();
        let interest_now: f64 = data.debts.iter().map(Debt::monthly_interest).sum();
        let free = match p.chosen.debt_free {
            _ if active == 0 => view! { <Stat label="Debt-free" value="Now" tone="good" sub="No debt to pay off" /> }.into_any(),
            Some(d) => view! { <Stat label="Debt-free" value=month_label(d) tone="good" sub=format!("In {} with this plan", months_text(p.chosen.months.len())) /> }.into_any(),
            None => view! { <Stat label="Debt-free" value="Not on this plan" tone="bad" sub="Payments don't cover the interest. Raise the extra payment." /> }.into_any(),
        };
        let saved = match (p.minimums_only.debt_free, p.chosen.debt_free) {
            (Some(_), Some(_)) => format!(
                "Saves {} and {} vs. minimums only",
                money((p.minimums_only.total_interest - p.chosen.total_interest).max(0.0), &sym),
                months_text(p.minimums_only.months.len().saturating_sub(p.chosen.months.len()))
            ),
            (None, Some(_)) => "Minimums alone would never pay it all off".to_string(),
            _ => String::new(),
        };
        let debt_tone = if data.total_debt() > 0.0 { "bad" } else { "good" };
        let debt_sub = format!("{active} debt{}", if active == 1 { "" } else { "s" });
        view! {
            <div class="grid grid-cols-2 gap-4 @3xl:grid-cols-4">
                <Stat label="Total debt" value=money(data.total_debt(), &sym) sub=debt_sub tone=debt_tone />
                <Stat label="Interest this month" value=money_cents(interest_now, &sym) sub=format!("About {} a year at today's balances", money(interest_now * 12.0, &sym)) />
                {free}
                <Stat label="Total interest" value=money(p.chosen.total_interest, &sym) sub=saved />
            </div>
        }
    };

    let main = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        if data.debts.is_empty() {
            return view! { <EmptyState title="No debts" text="Add credit cards and loans to see what they cost each month and get a plan to pay them off." /> }.into_any();
        }
        let p = plans.get();
        let order = plan_order(&data.debts, &p);
        let target = order.iter().copied().find(|&i| data.debts[i].balance > 0.0);
        let account_options: Vec<(String, String)> = std::iter::once((String::new(), "The debt's paying account".to_string()))
            .chain(data.accounts.iter().map(|a| (a.id.to_string(), a.name.clone())))
            .collect();
        let cards = order
            .into_iter()
            .enumerate()
            .map(|(pos, idx)| {
                let d = &data.debts[idx];
                let pay_from = d.account_id.and_then(|a| data.account_name(a)).map(str::to_string)
                    .unwrap_or_else(|| data.primary_cash_account().map(|a| a.name.clone()).unwrap_or_default());
                let payments: Vec<DebtPayment> = data.debt_payments.iter().filter(|x| x.debt_id == d.id).cloned().collect();
                let set_aside = data.buckets.iter().find(|b| b.debt_id == Some(d.id)).map(|b| (b.balance, data.account_name(b.account_id).unwrap_or("").to_string()));
                let has_bucket = set_aside.is_some();
                view! {
                    <DebtCard debt=d.clone() position=pos + 1 plans=p.clone() index=idx is_target=target == Some(idx) sym=sym.clone() pay_from=pay_from
                        payments=payments account_options=account_options.clone() set_aside=set_aside has_bucket=has_bucket form=form today=today />
                }
            })
            .collect_view();
        let history: Vec<DebtPayment> = data.debt_payments.iter().filter(|x| x.logged).cloned().collect();
        let history_view = (!history.is_empty()).then(|| view! { <PaymentHistory payments=history data=data.clone() sym=sym.clone() /> });
        view! {
            <PayoffChart plan=p.chosen.clone() debts=data.debts.clone() sym=sym.clone() />
            <Card>
                <CardHeader>
                    <CardTitle>"Debts"</CardTitle>
                    <CardDescription>"In payoff order. The first one still owed gets the extra payment."</CardDescription>
                </CardHeader>
                <CardContent class="flex flex-col gap-3">{cards}</CardContent>
            </Card>
            <Schedule plan=p.chosen.clone() debts=data.debts.clone() sym=sym.clone() />
            {history_view}
        }
        .into_any()
    };

    view! {
        <PageHeader title="Debt payoff" subtitle="Credit cards and loans: what they cost, and a plan to pay them off while still covering rent and your bills." />
        {stats}
        {move || store.snap.with(|s| s.as_ref().is_some_and(|s| !s.data.debts.is_empty())).then(|| view! { <Budget /> })}
        <div class="grid gap-6 @5xl:grid-cols-3">
            <div class="flex flex-col gap-6 @5xl:col-span-2">{main}</div>
            <DebtFormCard form=form />
        </div>
    }
}

/// Income minus bills, expenses and savings = what the plan may use.
#[component]
fn Budget() -> impl IntoView {
    let store = use_store();
    let today = store.today();
    let extra_text = RwSignal::new(store.snap.with_untracked(|s| s.as_ref().map(|s| num_str(s.data.settings.debt_extra_monthly.max(0.0))).unwrap_or_default()));
    let numbers = Memo::new(move |_| {
        let data = store.data();
        let income = data.monthly_total(|k| k == FlowKind::Income, today);
        let bills = data.monthly_total(|k| k == FlowKind::Bill, today);
        let expenses = data.monthly_total(|k| k == FlowKind::Expense, today);
        let saving = data.monthly_cash_contributions();
        let minimums: f64 = data.debts.iter().filter(|d| d.balance > 0.0).map(|d| d.min_payment.max(0.0)).sum();
        (income, bills, expenses, saving, minimums, data.settings.debt_extra_monthly.max(0.0), data.settings.debt_strategy)
    });
    let suggestion = move || numbers.with(|n| (n.0 - n.1 - n.2 - n.3 - n.4).max(0.0));
    // Save when the person stops typing a valid number (on change), keeping the box as typed.
    let commit = move || {
        if let Some(x) = parse_num(&extra_text.get_untracked()) {
            save_settings(store, |s| s.debt_extra_monthly = (x.max(0.0) * 100.0).round() / 100.0);
        }
    };
    let lines = move || {
        let sym = store.currency();
        let (income, bills, expenses, saving, minimums, extra, _) = numbers.get();
        let available = income - bills - expenses - saving;
        let after_plan = available - minimums - extra;
        let warning = if available < minimums {
            Some(view! { <p class="mt-2 rounded-md bg-destructive-light px-3 py-2 text-xs text-destructive-dark">
                {format!("Your minimum payments are {} more than what's left after bills and expenses. Look for expenses to cut, or ask lenders about lower payments.", money(minimums - available, &sym))}
            </p> }.into_any())
        } else if after_plan < 0.0 {
            Some(view! { <p class="mt-2 rounded-md bg-warning-light px-3 py-2 text-xs text-warning-dark">
                {format!("This plan needs {} more per month than you have after rent, bills and expenses. Lower the extra payment to at most {}.", money(-after_plan, &sym), money(suggestion(), &sym))}
            </p> }.into_any())
        } else {
            None
        };
        view! {
            <BudgetLine label="Take-home income" amount=income sym=sym.clone() sign="+" />
            <BudgetLine label="Bills (rent, utilities, insurance…)" amount=bills sym=sym.clone() sign="−" />
            <BudgetLine label="Recurring expenses" amount=expenses sym=sym.clone() sign="−" />
            {(saving > 0.0).then(|| view! { <BudgetLine label="Savings & investing from checking" amount=saving sym=sym.clone() sign="−" /> })}
            <div class="my-1 border-t"></div>
            <BudgetLine label="Available for debt" amount=available sym=sym.clone() sign="=" strong=true />
            <BudgetLine label="Minimum payments" amount=minimums sym=sym.clone() sign="−" />
            <BudgetLine label="Extra toward debt" amount=extra sym=sym.clone() sign="−" />
            <div class="my-1 border-t"></div>
            <BudgetLine label="Left over after the plan" amount=after_plan sym=sym.clone() sign="=" strong=true tone=if after_plan < 0.0 { "bad" } else { "good" } />
            {warning}
        }
    };
    let strategy = move || numbers.with(|n| n.6);
    view! {
        <Card>
            <CardHeader>
                <CardTitle>"Your monthly budget"</CardTitle>
                <CardDescription>"Rent, utilities and your other bills come first. The plan only uses what's left."</CardDescription>
            </CardHeader>
            <CardContent class="grid gap-6 @3xl:grid-cols-2">
                <div class="flex flex-col gap-1 text-sm">{lines}</div>
                <div class="flex flex-col gap-4">
                    <div class="flex flex-col gap-1.5">
                        <label class="text-xs font-medium text-muted-foreground">{move || format!("Extra toward debt each month ({})", store.currency())}</label>
                        <div class="flex gap-2">
                            <input type="text" inputmode="decimal" class=crate::ui::INPUT_CLASS placeholder="0" aria-label="Extra toward debt each month"
                                prop:value=move || extra_text.get() on:input=move |ev| extra_text.set(event_target_value(&ev)) on:change=move |_| commit() />
                            <Button variant=ButtonVariant::Outline title="Use everything left after bills, expenses and minimums"
                                on_click=move |_| { let v = suggestion().floor(); extra_text.set(num_str(v)); save_settings(store, |s| s.debt_extra_monthly = v); }>
                                {move || format!("Use {}", money(suggestion().floor(), &store.currency()))}
                            </Button>
                        </div>
                        <p class="text-xs text-muted-foreground">{move || format!("On top of the minimums. Up to {} keeps every bill covered.", money(suggestion(), &store.currency()))}</p>
                    </div>
                    <div class="flex flex-col gap-2">
                        <span class="text-xs font-medium text-muted-foreground">"Which debt gets the extra first"</span>
                        <div class="grid grid-cols-2 gap-2">
                            <StrategyButton selected=Signal::derive(move || strategy() == Strategy::Avalanche) title="Avalanche" text="Highest interest rate first. Pays the least interest."
                                on_click=move |_| save_settings(store, |s| s.debt_strategy = Strategy::Avalanche) />
                            <StrategyButton selected=Signal::derive(move || strategy() == Strategy::Snowball) title="Snowball" text="Smallest balance first. Quick wins keep you motivated."
                                on_click=move |_| save_settings(store, |s| s.debt_strategy = Strategy::Snowball) />
                        </div>
                        <p class="text-xs text-muted-foreground">
                            "When a debt is paid off, its payment rolls into the next one, so the monthly total stays the same until you're debt-free."
                        </p>
                    </div>
                </div>
            </CardContent>
        </Card>
    }
}

#[component]
fn BudgetLine(#[prop(into)] label: String, amount: f64, sym: String, #[prop(into)] sign: String, #[prop(optional)] strong: bool, #[prop(into, optional)] tone: String) -> impl IntoView {
    let color = match tone.as_str() {
        "good" => "text-success",
        "bad" => "text-destructive",
        _ => "",
    };
    let weight = if strong { "font-semibold" } else { "text-muted-foreground" };
    view! {
        <div class=format!("flex items-center justify-between gap-3 {weight}")>
            <span>{label}</span>
            <span class=format!("tabular-nums {color}")>{format!("{sign} {}", money(amount.abs(), &sym))}</span>
        </div>
    }
}

#[component]
fn StrategyButton(#[prop(into)] selected: Signal<bool>, #[prop(into)] title: String, #[prop(into)] text: String, #[prop(into)] on_click: Callback<()>) -> impl IntoView {
    view! {
        <button type="button" aria-pressed=move || selected.get().to_string()
            class=move || if selected.get() { "flex flex-col items-start gap-1 rounded-lg border-2 border-primary p-3 text-left" } else { "flex flex-col items-start gap-1 rounded-lg border p-3 text-left hover:bg-accent cursor-pointer" }
            on:click=move |_| on_click.run(())>
            <span class="text-sm font-semibold">{title}</span>
            <span class="text-xs text-muted-foreground">{text}</span>
        </button>
    }
}

#[component]
fn DebtCard(
    debt: Debt,
    position: usize,
    plans: Plans,
    index: usize,
    is_target: bool,
    sym: String,
    pay_from: String,
    payments: Vec<DebtPayment>,
    account_options: Vec<(String, String)>,
    set_aside: Option<(f64, String)>,
    has_bucket: bool,
    form: RwSignal<DebtForm>,
    today: NaiveDate,
) -> impl IntoView {
    let store = use_store();
    let paying = RwSignal::new(false);
    let id = debt.id;
    let d_edit = debt.clone();
    let mut planned: Vec<DebtPayment> = payments.iter().filter(|p| !p.logged).cloned().collect();
    planned.sort_by_key(|p| p.date);
    let logged: Vec<DebtPayment> = payments.iter().filter(|p| p.logged).cloned().collect();
    let paid_extra: f64 = logged.iter().map(|p| p.applied).sum();
    let interest = debt.monthly_interest();
    let covers_interest = debt.min_payment > interest + 0.005 || debt.balance <= 0.0;
    let terms = format!("{}% APR · minimum {}/mo · from {pay_from}", num_str(debt.apr), money_cents(debt.min_payment, &sym));
    let (balance, interest_text) = (money(debt.balance, &sym), format!("{} interest/mo", money_cents(interest, &sym)));
    let warn = (!covers_interest).then(|| view! {
        <p class="rounded-md bg-destructive-light px-3 py-2 text-xs text-destructive-dark">
            {format!("The minimum payment doesn't cover the {} of interest charged each month, so this balance keeps growing unless you pay more.", money_cents(interest, &sym))}
        </p>
    });
    let with_plan = match plans.chosen.payoff.get(index).copied().flatten() {
        Some(d) => view! { <div>{format!("Paid off {} · interest {}", month_label(d), money(plans.chosen.interest_by_debt[index], &sym))}</div> }.into_any(),
        None => view! { <div class="text-destructive">"Not paid off within 50 years"</div> }.into_any(),
    };
    let only_min = match plans.minimums_only.payoff.get(index).copied().flatten() {
        Some(d) => view! { <div>{format!("Paid off {} · interest {}", month_label(d), money(plans.minimums_only.interest_by_debt[index], &sym))}</div> }.into_any(),
        None => view! { <div class="text-destructive">"Never paid off"</div> }.into_any(),
    };
    let planned_rows = planned.into_iter().map(|pp| {
        let pid = pp.id;
        let text = format!("{} on {}{}", money_cents(pp.amount, &sym), pp.date.format("%b %-d, %Y"), note_suffix(&pp.notes));
        let past = pp.date < today;
        view! {
            <div class="flex flex-wrap items-center gap-2">
                {if past {
                    view! {
                        <Badge variant=BadgeVariant::Warning>"Was planned"</Badge>
                        <span class="flex-1">{format!("{text}. Did you make it?")}</span>
                        <Button size=ButtonSize::Sm variant=ButtonVariant::Outline
                            on_click=move |_| store.apply(Change::LogDebtPayment { payment: pp.clone(), deduct: true }, "Payment logged")>"Log it"</Button>
                    }.into_any()
                } else {
                    view! { <Badge variant=BadgeVariant::Info>"Planned"</Badge><span class="flex-1">{text}</span> }.into_any()
                }}
                <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost on_click=move |_| store.apply(Change::DeleteDebtPayment(pid), "Planned payment removed")>"Remove"</Button>
            </div>
        }
    }).collect_view();
    let has_payments = !logged.is_empty() || !payments.iter().all(|p| p.logged);
    let logged_rows = logged.into_iter().take(3).map(|lp| {
        let pid = lp.id;
        view! {
            <div class="flex flex-wrap items-center gap-2">
                <Badge variant=BadgeVariant::Success>"Paid"</Badge>
                <span class="flex-1">{format!("{} on {}{}", money_cents(lp.amount, &sym), lp.date.format("%b %-d, %Y"), note_suffix(&lp.notes))}</span>
                <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost title="Undo: puts the balances back"
                    on_click=move |_| store.apply(Change::DeleteDebtPayment(pid), "Payment undone")>"Undo"</Button>
            </div>
        }
    }).collect_view();
    let payments_box = has_payments.then(|| view! {
        <div class="flex flex-col gap-1 rounded-md border px-3 py-2 text-xs">
            {planned_rows}
            {logged_rows}
            {(paid_extra > 0.0).then(|| view! { <span class="text-muted-foreground">{format!("One-time payments so far: {}", money(paid_extra, &sym))}</span> })}
        </div>
    });
    let owed = debt.balance > 0.0;
    let set_aside_view = match set_aside {
        Some((amount, account)) => Some(view! {
            <span class="text-muted-foreground">
                "Set aside: " <b class="text-foreground">{money_cents(amount, &sym)}</b>
                {format!(" this month in {account} › {} payment (follows your plan)", debt.name)}
            </span>
        }.into_any()),
        None if owed => Some(view! {
            <Button size=ButtonSize::Sm variant=ButtonVariant::Outline title="Create a bucket in the paying account that holds this debt's planned payment"
                on_click=move |_| store.apply(Change::AddDebtBucket(id), "Payments now set aside in a bucket")>"Set aside payments in a bucket"</Button>
        }.into_any()),
        None => None,
    };
    let fixed = owed.then(|| {
        let items = [12u32, 24, 36].into_iter().map(|m| {
            let pay = money(payment_for_months(debt.balance, debt.apr, m), &sym);
            view! { <span class="tabular-nums">{format!("{} yr: ", m / 12)}<b class="text-foreground">{pay}</b></span> }
        }).collect_view();
        view! { <div class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground"><span>"Monthly payment to be debt-free in:"</span>{items}</div> }
    });
    let pay_debt = debt.clone();
    let pay_sym = sym.clone();
    view! {
        <div class=move || if form.with(|f| f.id) == id { "rounded-lg border border-ring p-4 flex flex-col gap-3" } else { "rounded-lg border p-4 flex flex-col gap-3" }>
            <div class="flex items-start gap-3">
                <div class="flex size-7 shrink-0 items-center justify-center rounded-full bg-secondary text-xs font-semibold">{position.to_string()}</div>
                <div class="flex min-w-0 flex-1 flex-col gap-1">
                    <div class="flex flex-wrap items-center gap-2">
                        <span class="font-medium">{debt.name.clone()}</span>
                        <Badge>{debt.kind.label()}</Badge>
                        {is_target.then(|| view! { <Badge variant=BadgeVariant::Info>"Gets the extra payment"</Badge> })}
                    </div>
                    <div class="text-xs text-muted-foreground">{terms}</div>
                </div>
                <div class="text-right">
                    <div class="text-lg font-semibold tabular-nums">{balance}</div>
                    <div class="text-xs text-muted-foreground tabular-nums">{interest_text}</div>
                </div>
                <div class="flex shrink-0">
                    <RowActions edit_title="Edit (e.g. update the balance)"
                        on_edit=move |_| form.set(DebtForm::from_debt(&d_edit, has_bucket))
                        on_delete=move |_| store.apply(Change::DeleteDebt(id), "Debt removed") />
                </div>
            </div>
            {warn}
            <div class="grid gap-2 text-xs @2xl:grid-cols-2">
                <div class="rounded-md bg-muted/60 px-3 py-2"><div class="font-medium text-foreground">"With your plan"</div>{with_plan}</div>
                <div class="rounded-md bg-muted/60 px-3 py-2"><div class="font-medium text-foreground">"Paying only the minimum"</div>{only_min}</div>
            </div>
            {payments_box}
            {move || paying.get().then(|| view! {
                <PaymentForm debt=pay_debt.clone() account_options=account_options.clone() sym=pay_sym.clone() on_done=move |_| paying.set(false) />
            })}
            <div class="flex flex-wrap items-center gap-2 text-xs">
                {move || (owed && !paying.get()).then(|| view! {
                    <Button size=ButtonSize::Sm variant=ButtonVariant::Outline on_click=move |_| paying.set(true)><Icon name=IconName::Plus />"Log a payment"</Button>
                })}
                {set_aside_view}
            </div>
            {fixed}
        </div>
    }
}

/// Each debt's balance until debt-free, stacked.
#[component]
fn PayoffChart(plan: PayoffPlan, debts: Vec<Debt>, sym: String) -> impl IntoView {
    if plan.months.is_empty() {
        return ().into_any();
    }
    let n = plan.months.len();
    // Up to ~120 points: monthly for short plans, every few months for long ones.
    let stride = n.div_ceil(120).max(1);
    let mut idx: Vec<usize> = (0..n).step_by(stride).collect();
    if idx.last() != Some(&(n - 1)) {
        idx.push(n - 1);
    }
    let labels: Vec<String> = idx.iter().map(|&i| plan.months[i].month.format("%b %y").to_string()).collect();
    const COLORS: [&str; 6] = ["var(--destructive)", "var(--warning)", "var(--chart-3)", "var(--chart-5)", "var(--info)", "var(--success)"];
    let layers: Vec<Series> = plan
        .order
        .iter()
        .rev()
        .enumerate()
        .filter(|(_, i)| **i < debts.len())
        .map(|(c, &i)| Series::new(debts[i].name.clone(), COLORS[c % COLORS.len()], idx.iter().map(|&k| plan.months[k].balances[i]).collect(), false))
        .collect();
    let total = Series::new("Total owed", "var(--foreground)", idx.iter().map(|&k| plan.months[k].balances.iter().sum()).collect(), true);
    let description = match plan.debt_free {
        Some(d) => format!("Debt-free in {}, with {} of interest along the way.", month_label(d), money(plan.total_interest, &sym)),
        None => "At these payments the debt isn't paid off within 50 years.".to_string(),
    };
    view! {
        <Card>
            <CardHeader><CardTitle>"Payoff plan"</CardTitle><CardDescription>{description}</CardDescription></CardHeader>
            <CardContent><StackedChart layers=layers total=total labels=labels currency=sym /></CardContent>
        </Card>
    }
    .into_any()
}

/// Month-by-month payments.
#[component]
fn Schedule(plan: PayoffPlan, debts: Vec<Debt>, sym: String) -> impl IntoView {
    let show_all = RwSignal::new(false);
    let n = plan.months.len();
    let heads = debts.iter().map(|d| d.name.clone()).map(|n| view! { <TableHead class="text-right">{n}</TableHead> }).collect_view();
    let plan = StoredValue::new(plan);
    let debt_count = debts.len();
    let rows = move || {
        let shown = if show_all.get() { n } else { n.min(12) };
        plan.with_value(|plan| {
            plan.months.iter().take(shown).map(|m| {
                let cells: Vec<(&'static str, String)> = (0..debt_count).map(|i| {
                    let paid_off = plan.payoff.get(i).copied().flatten() == Some(m.month);
                    let pay = m.payments.get(i).copied().unwrap_or(0.0);
                    let class = if paid_off { "text-right tabular-nums font-semibold text-success" } else { "text-right tabular-nums" };
                    let text = if pay > 0.005 { format!("{}{}", money_cents(pay, &sym), if paid_off { " ✓" } else { "" }) } else { "—".into() };
                    (class, text)
                }).collect();
                let cells = cells.into_iter().map(|(class, text)| view! { <TableCell class=class>{text}</TableCell> }).collect_view();
                let month = month_label(m.month);
                let (interest, owed) = (money_cents(m.interest.iter().sum(), &sym), money(m.balances.iter().sum(), &sym));
                view! {
                    <TableRow>
                        <TableCell class="whitespace-nowrap font-medium">{month}</TableCell>
                        {cells}
                        <TableCell class="text-right tabular-nums text-muted-foreground">{interest}</TableCell>
                        <TableCell class="text-right tabular-nums">{owed}</TableCell>
                    </TableRow>
                }
            }).collect_view()
        })
    };
    view! {
        <Card>
            <CardHeader>
                <CardTitle>"Payment schedule"</CardTitle>
                <CardDescription>"What to pay each month. Each payment comes from the account chosen for that debt."</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-3">
                <TableWrapper class="max-h-[30rem]">
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>"Month"</TableHead>
                                {heads}
                                <TableHead class="text-right">"Interest"</TableHead>
                                <TableHead class="text-right">"Still owed"</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>{rows}</TableBody>
                    </Table>
                </TableWrapper>
                {(n > 12).then(|| view! {
                    <div>
                        <Button variant=ButtonVariant::Link size=ButtonSize::Sm class="px-0" on_click=move |_| show_all.update(|v| *v = !*v)>
                            {move || if show_all.get() { "Show the first 12 months".to_string() } else { format!("Show all {n} months") }}
                        </Button>
                    </div>
                })}
            </CardContent>
        </Card>
    }
}

/// Every one-time payment that was made, newest first, with undo.
#[component]
fn PaymentHistory(payments: Vec<DebtPayment>, data: AppData, sym: String) -> impl IntoView {
    let store = use_store();
    let total: f64 = payments.iter().map(|p| p.applied).sum();
    let rows = payments.into_iter().map(|p| {
        let pid = p.id;
        let debt = data.debts.iter().find(|d| d.id == p.debt_id).map(|d| d.name.clone()).unwrap_or_default();
        let amount = money_cents(p.amount, &sym);
        let from = format!("{}{}", p.account_id.and_then(|a| data.account_name(a)).unwrap_or(""), if p.deducted { "" } else { " (balance not changed)" });
        view! {
            <TableRow>
                <TableCell class="whitespace-nowrap">{p.date.format("%b %-d, %Y").to_string()}</TableCell>
                <TableCell>{debt}</TableCell>
                <TableCell class="text-right tabular-nums">{amount}</TableCell>
                <TableCell class="text-muted-foreground">{from}</TableCell>
                <TableCell class="text-muted-foreground">{p.notes.clone()}</TableCell>
                <TableCell class="text-right">
                    <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost on_click=move |_| store.apply(Change::DeleteDebtPayment(pid), "Payment undone")>"Undo"</Button>
                </TableCell>
            </TableRow>
        }
    }).collect_view();
    view! {
        <Card>
            <CardHeader>
                <CardTitle>"Payment history"</CardTitle>
                <CardDescription>{format!("One-time payments you've logged: {} off your debt so far. Undo puts the balances back.", money(total, &sym))}</CardDescription>
            </CardHeader>
            <CardContent>
                <TableWrapper class="max-h-[24rem]">
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>"Date"</TableHead>
                                <TableHead>"Debt"</TableHead>
                                <TableHead class="text-right">"Amount"</TableHead>
                                <TableHead>"From"</TableHead>
                                <TableHead>"Note"</TableHead>
                                <TableHead>""</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>{rows}</TableBody>
                    </Table>
                </TableWrapper>
            </CardContent>
        </Card>
    }
}

#[derive(Clone, PartialEq)]
struct PaymentDraft {
    amount: String,
    date: String,
    account: String,
    deduct: bool,
    notes: String,
}

/// Inline form to log a one-time payment (today or earlier) or schedule one (future date).
#[component]
fn PaymentForm(debt: Debt, account_options: Vec<(String, String)>, sym: String, #[prop(into)] on_done: Callback<()>) -> impl IntoView {
    let store = use_store();
    let today = store.today();
    let draft = RwSignal::new(PaymentDraft { amount: String::new(), date: today.format("%Y-%m-%d").to_string(), account: String::new(), deduct: true, notes: String::new() });
    let error = RwSignal::new(None::<String>);
    let future = move || draft.with(|d| parse_date(&d.date).is_some_and(|x| x > today));
    let debt_id = debt.id;
    let debt_account = debt.account_id;
    // Name the default choice after the account it stands for.
    let usual = store.snap.with_untracked(|s| {
        s.as_ref().and_then(|s| {
            debt_account.and_then(|id| s.data.account_name(id).map(str::to_string)).or_else(|| s.data.primary_cash_account().map(|a| a.name.clone()))
        })
    });
    let mut account_options = account_options;
    if let (Some(first), Some(name)) = (account_options.first_mut(), usual) {
        first.1 = format!("{name} (usual)");
    }
    let account_name = move || {
        store.snap.with(|s| {
            s.as_ref()
                .and_then(|s| {
                    draft.with(|d| d.account.parse::<i64>().ok())
                        .or(debt_account)
                        .and_then(|id| s.data.account_name(id).map(str::to_string))
                        .or_else(|| s.data.primary_cash_account().map(|a| a.name.clone()))
                })
                .unwrap_or_else(|| "the account".into())
        })
    };
    let submit = move |_| {
        let d = draft.get_untracked();
        let Some(amount) = parse_num(&d.amount).filter(|v| *v > 0.0) else { return error.set(Some("Enter the amount you paid.".into())) };
        let Some(date) = parse_date(&d.date) else { return error.set(Some("Pick the date of the payment.".into())) };
        let p = DebtPayment { id: 0, debt_id, date, amount, account_id: d.account.parse().ok(), notes: d.notes.trim().to_string(), logged: false, applied: 0.0, deducted: false };
        if date > today {
            store.apply(Change::PlanDebtPayment(p), "Payment scheduled; your plan now includes it");
        } else {
            store.apply(Change::LogDebtPayment { payment: p, deduct: d.deduct }, "Payment logged");
        }
        on_done.run(());
    };
    view! {
        <div class="flex flex-col gap-3 rounded-md border bg-muted/40 p-3">
            <div class="text-sm font-medium">{format!("One-time payment toward {}", debt.name)}</div>
            <div class="grid grid-cols-2 gap-3 @2xl:grid-cols-3">
                <Field label=format!("Amount ({sym})")>
                    <FormInput form=draft get=|d: &PaymentDraft| d.amount.clone() set=|d: &mut PaymentDraft, v| d.amount = v placeholder="e.g. 500" inputmode="decimal" />
                </Field>
                <Field label="Date">
                    <FormInput form=draft kind="date" get=|d: &PaymentDraft| d.date.clone() set=|d: &mut PaymentDraft, v| d.date = v />
                </Field>
                <Field label="Paid from">
                    <NativeSelect label="Paid from" value=Signal::derive(move || draft.with(|d| d.account.clone())) options=account_options
                        on_change=move |v: String| draft.update(|d| d.account = v) />
                </Field>
            </div>
            <Field label="Note">
                <FormInput form=draft get=|d: &PaymentDraft| d.notes.clone() set=|d: &mut PaymentDraft, v| d.notes = v placeholder="Optional, e.g. tax refund" />
            </Field>
            {move || if future() {
                view! {
                    <p class="text-xs text-muted-foreground">
                        "This date is in the future, so it's scheduled: your payoff plan and projections include it now, and you'll be asked to confirm it once the date passes."
                    </p>
                }.into_any()
            } else {
                view! {
                    <SettingRow title=format!("Also subtract it from {}'s balance", account_name()) caption="Turn off if you've already updated that account's balance.">
                        <FormSwitch form=draft get=|d: &PaymentDraft| d.deduct set=|d: &mut PaymentDraft, v| d.deduct = v label="Subtract from account balance" />
                    </SettingRow>
                }.into_any()
            }}
            {move || error.get().map(|e| view! { <p class="text-xs text-destructive">{e}</p> })}
            <div class="flex gap-2">
                <Button size=ButtonSize::Sm on_click=submit>{move || if future() { "Schedule payment" } else { "Log payment" }}</Button>
                <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost on_click=move |_| on_done.run(())>"Cancel"</Button>
            </div>
        </div>
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
fn DebtFormCard(form: RwSignal<DebtForm>) -> impl IntoView {
    let store = use_store();
    let error = RwSignal::new(None::<String>);
    let editing = move || form.with(|f| f.id != 0);
    let kind_options: Vec<(String, String)> = DebtKind::ALL.iter().map(|k| (k.key().to_string(), k.label().to_string())).collect();
    let account_options = move || {
        let data = store.data();
        let mut opts = vec![(String::new(), match data.primary_cash_account() {
            Some(a) => format!("Default ({})", a.name),
            None => "Default".into(),
        })];
        opts.extend(data.accounts.iter().map(|a| (a.id.to_string(), a.name.clone())));
        opts
    };
    let set_aside_hint = move || {
        let name = form.with(|f| f.name.trim().to_string());
        if name.is_empty() {
            "Adds a payment bucket to the paying account that always holds this debt's planned payment, so goals and spending won't use it.".to_string()
        } else {
            format!("Adds a \u{201c}{name} payment\u{201d} bucket to the paying account that always holds this debt's planned payment, so goals and spending won't use it.")
        }
    };
    // Live preview from what's typed so far.
    let preview = move || {
        let sym = store.currency();
        form.with(DebtForm::to_debt).ok().filter(|d| d.balance > 0.0).map(|d| {
            let only_min = plan(std::slice::from_ref(&d), Strategy::Avalanche, 0.0, false, store.today(), MAX_MONTHS);
            let line = match only_min.debt_free {
                Some(free) => format!("At the minimum: paid off {} ({}), {} interest.", month_label(free), months_text(only_min.months.len()), money(only_min.total_interest, &sym)),
                None => "At the minimum this is never paid off.".to_string(),
            };
            view! {
                <div class="rounded-md bg-muted/60 px-3 py-2 text-xs text-muted-foreground">
                    "Interest this month: " <b class="text-foreground">{money_cents(d.monthly_interest(), &sym)}</b>
                    <br />
                    {line}
                </div>
            }
        })
    };
    let save = move |_| match form.with_untracked(DebtForm::to_debt) {
        Ok(d) => {
            error.set(None);
            let set_aside = form.with_untracked(|f| f.set_aside);
            let msg = match (d.id == 0, set_aside) {
                (true, true) => "Debt added, with a bucket for its payments",
                (true, false) => "Debt added",
                _ => "Debt saved",
            };
            store.apply(Change::SaveDebt { debt: d, set_aside }, msg);
            form.set(DebtForm::new());
        }
        Err(e) => error.set(Some(e)),
    };
    view! {
        <Card>
            <CardHeader>
                <CardTitle>{move || if editing() { "Edit debt" } else { "Add a debt" }}</CardTitle>
                <CardDescription>"Find the balance, APR and minimum payment on your latest statement."</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-4">
                <Field label="Type">
                    <NativeSelect label="Debt type" value=Signal::derive(move || form.with(|f| f.kind.key().to_string())) options=kind_options
                        on_change=move |v: String| if let Some(k) = DebtKind::from_key(&v) { form.update(|f| f.kind = k) } />
                </Field>
                <Field label="Name">
                    <FormInput form=form get=|f: &DebtForm| f.name.clone() set=|f: &mut DebtForm, v| f.name = v placeholder="e.g. Visa card" />
                </Field>
                <Field label=format!("Balance owed ({})", store.currency())>
                    <FormInput form=form get=|f: &DebtForm| f.balance.clone() set=|f: &mut DebtForm, v| f.balance = v placeholder="0" inputmode="decimal" />
                </Field>
                <div class="grid grid-cols-2 items-end gap-3">
                    <Field label="Interest rate (APR %)">
                        <FormInput form=form get=|f: &DebtForm| f.apr.clone() set=|f: &mut DebtForm, v| f.apr = v placeholder="e.g. 24.99" inputmode="decimal" />
                    </Field>
                    <Field label=format!("Minimum payment ({}/mo)", store.currency())>
                        <FormInput form=form get=|f: &DebtForm| f.min_payment.clone() set=|f: &mut DebtForm, v| f.min_payment = v placeholder="0" inputmode="decimal" />
                    </Field>
                </div>
                <Field label="Paid from">
                    {move || view! {
                        <NativeSelect label="Paid from" value=Signal::derive(move || form.with(|f| f.account.clone())) options=account_options()
                            on_change=move |v: String| form.update(|f| f.account = v) />
                    }}
                </Field>
                {preview}
                <SettingRow title="Set aside payments in a bucket">
                    <FormSwitch form=form get=|f: &DebtForm| f.set_aside set=|f: &mut DebtForm, v| f.set_aside = v label="Set aside payments in a bucket" />
                </SettingRow>
                <p class="-mt-2 text-xs text-muted-foreground">{set_aside_hint}</p>
                <Field label="Notes">
                    <FormInput form=form get=|f: &DebtForm| f.notes.clone() set=|f: &mut DebtForm, v| f.notes = v placeholder="Optional" />
                </Field>
                {move || error.get().map(|e| view! { <p class="text-sm text-destructive">{e}</p> })}
                <div class="flex gap-2">
                    <Button on_click=save><Icon name=IconName::Plus />{move || if editing() { "Save changes" } else { "Add debt" }}</Button>
                    {move || editing().then(|| view! { <Button variant=ButtonVariant::Outline on_click=move |_| { error.set(None); form.set(DebtForm::new()) }>"Cancel"</Button> })}
                </div>
                <p class="text-xs text-muted-foreground">"Update the balance from your statement now and then; the plan recalculates from there."</p>
            </CardContent>
        </Card>
    }
}
