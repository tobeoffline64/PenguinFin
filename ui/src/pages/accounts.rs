//! Accounts & investments: accounts by group, buckets inside them, and holdings. The rules for
//! which fields each account type has are the same as 1.x (and keep its tests).

use crate::app::use_store;
use crate::ui::icon::{Icon, IconName};
use crate::ui::*;
use leptos::prelude::*;
use penguinfin_core::format::{money, money_cents, num_str, parse_num};
use penguinfin_core::fred::{Indicator, MarketData};
use penguinfin_core::ipc::Change;
use penguinfin_core::model::{Account, AccountGroup, AccountKind, Bucket, Holding};

/// Where an account's monthly contribution comes from.
#[derive(Clone, Copy, PartialEq)]
enum Funding {
    /// Moved out of checking each month.
    FromChecking,
    /// Withheld from the paycheck (take-home pay already excludes it).
    Payroll,
    /// Either; the user picks, with this default (true = from checking).
    Choice(bool),
}

/// Which fields the form shows for an account type, and how they're labelled.
struct KindSpec {
    noun: &'static str,
    intro: &'static str,
    name_placeholder: &'static str,
    balance_label: &'static str,
    balance_hint: Option<&'static str>,
    /// `None` hides the field (return is 0).
    return_field: Option<(&'static str, &'static str)>,
    /// `None` hides contributions entirely.
    contrib_field: Option<(&'static str, &'static str)>,
    funding: Funding,
    match_label: Option<&'static str>,
}

/// Quick picks for an account's purpose.
const PURPOSES: [&str; 9] =
    ["Everyday spending", "Bills", "Emergency fund", "Medical", "Food", "Car", "Home", "Travel", "Gifts"];

const MARKET_HINT: &str = "Long-run stock market averages are often assumed at 6–8% a year.";

fn spec(kind: AccountKind) -> KindSpec {
    match kind {
        AccountKind::Checking => KindSpec {
            noun: "checking account",
            intro: "Your everyday account. In projections, money left over each month collects in your first checking account.",
            name_placeholder: "e.g. Chase checking",
            balance_label: "Current balance",
            balance_hint: None,
            return_field: None,
            contrib_field: None,
            funding: Funding::FromChecking,
            match_label: None,
        },
        AccountKind::Savings => KindSpec {
            noun: "savings account",
            intro: "Emergency fund or high-yield savings, earning interest.",
            name_placeholder: "e.g. Ally high-yield savings",
            balance_label: "Current balance",
            balance_hint: None,
            return_field: Some(("Interest rate (APY) %", "Shown on your bank's website, e.g. 4.0")),
            contrib_field: Some(("Monthly transfer from checking", "Moved from checking into this account each month.")),
            funding: Funding::FromChecking,
            match_label: None,
        },
        AccountKind::Brokerage => KindSpec {
            noun: "brokerage account",
            intro: "A regular (taxable) investment account.",
            name_placeholder: "e.g. Fidelity brokerage",
            balance_label: "Cash / uninvested balance",
            balance_hint: Some("After saving, add stocks and funds as holdings, or enter the account's total value here instead."),
            return_field: Some(("Expected yearly return %", MARKET_HINT)),
            contrib_field: Some(("Monthly investment from checking", "Moved from checking and invested each month.")),
            funding: Funding::FromChecking,
            match_label: None,
        },
        AccountKind::Retirement401k => KindSpec {
            noun: "401(k) / 403(b)",
            intro: "Workplace retirement plan funded from your paycheck.",
            name_placeholder: "e.g. Work 401(k)",
            balance_label: "Current balance",
            balance_hint: None,
            return_field: Some(("Expected yearly return %", MARKET_HINT)),
            contrib_field: Some((
                "Your contribution / month",
                "Taken from your paycheck before take-home pay, so it doesn't come out of checking.",
            )),
            funding: Funding::Payroll,
            match_label: Some("Employer match / month"),
        },
        AccountKind::TraditionalIra | AccountKind::RothIra => KindSpec {
            noun: if kind == AccountKind::RothIra { "Roth IRA" } else { "traditional IRA" },
            intro: "Individual retirement account you fund yourself.",
            name_placeholder: if kind == AccountKind::RothIra { "e.g. Vanguard Roth IRA" } else { "e.g. Schwab traditional IRA" },
            balance_label: "Current balance",
            balance_hint: None,
            return_field: Some(("Expected yearly return %", MARKET_HINT)),
            contrib_field: Some(("Monthly contribution", "How much you put in each month.")),
            funding: Funding::Choice(true),
            match_label: None,
        },
        AccountKind::Hsa => KindSpec {
            noun: "HSA",
            intro: "Health savings account; can be invested for the long term.",
            name_placeholder: "e.g. HealthEquity HSA",
            balance_label: "Current balance",
            balance_hint: None,
            return_field: Some(("Expected yearly return %", "Use 0–1% if it stays in cash, or a market return if invested.")),
            contrib_field: Some(("Your contribution / month", "Usually through payroll; turn on the switch if you pay it from checking.")),
            funding: Funding::Choice(false),
            match_label: Some("Employer contribution / month"),
        },
        AccountKind::Pension => KindSpec {
            noun: "pension / TSP",
            intro: "Pension, Thrift Savings Plan, or similar employer plan.",
            name_placeholder: "e.g. TSP",
            balance_label: "Current balance",
            balance_hint: None,
            return_field: Some(("Expected yearly return %", MARKET_HINT)),
            contrib_field: Some(("Your contribution / month", "Taken from your paycheck before take-home pay.")),
            funding: Funding::Payroll,
            match_label: Some("Employer / agency contribution / month"),
        },
        AccountKind::Other => KindSpec {
            noun: "asset",
            intro: "Anything else you own that counts toward net worth: home equity, a car, crypto, collectibles.",
            name_placeholder: "e.g. Home equity",
            balance_label: "Current value",
            balance_hint: None,
            return_field: Some(("Expected yearly change %", "Use a negative number for things that lose value, like a car (e.g. -15).")),
            contrib_field: None,
            funding: Funding::FromChecking,
            match_label: None,
        },
    }
}

/// The return/APY hint, enriched with live FRED figures when they've been synced.
fn return_hint(kind: AccountKind, default: &str, market: &MarketData) -> String {
    let date = |d: chrono::NaiveDate| d.format("%b %-d").to_string();
    match kind {
        AccountKind::Savings => match market.get(Indicator::FedFunds) {
            Some(v) => format!(
                "The federal funds rate is {:.2}% (FRED, {}); high-yield savings usually pay a little below it.",
                v.value,
                date(v.as_of)
            ),
            None => default.to_string(),
        },
        AccountKind::Brokerage
        | AccountKind::Retirement401k
        | AccountKind::TraditionalIra
        | AccountKind::RothIra
        | AccountKind::Pension => match market.get(Indicator::Sp500Return10y) {
            Some(v) => format!(
                "The S&P 500 rose {:.1}% a year over the last 10 years (FRED; price only, dividends add ~1–2%). \
                 Long-run assumptions of 6–8% are common.",
                v.value
            ),
            None => default.to_string(),
        },
        _ => default.to_string(),
    }
}

#[derive(Clone, PartialEq)]
struct AccountForm {
    id: i64,
    name: String,
    kind: AccountKind,
    balance: String,
    ret: String,
    contrib: String,
    from_cash: bool,
    matched: String,
    notes: String,
    purpose: String,
    minimum: String,
}

impl AccountForm {
    fn new() -> Self {
        Self::blank(AccountKind::Checking)
    }
    fn blank(kind: AccountKind) -> Self {
        AccountForm {
            id: 0,
            name: String::new(),
            kind,
            balance: String::new(),
            ret: num_str(kind.default_return_pct()),
            contrib: "0".into(),
            from_cash: match spec(kind).funding {
                Funding::FromChecking => true,
                Funding::Payroll => false,
                Funding::Choice(d) => d,
            },
            matched: "0".into(),
            notes: String::new(),
            purpose: String::new(),
            minimum: String::new(),
        }
    }
    fn from_account(a: &Account) -> Self {
        AccountForm {
            id: a.id,
            name: a.name.clone(),
            kind: a.kind,
            balance: num_str(a.balance),
            ret: num_str(a.annual_return_pct),
            contrib: num_str(a.monthly_contribution),
            from_cash: a.contribution_from_cash,
            matched: num_str(a.employer_match_monthly),
            notes: a.notes.clone(),
            purpose: a.purpose.clone(),
            minimum: if a.minimum_balance > 0.0 { num_str(a.minimum_balance) } else { String::new() },
        }
    }
    /// Build the account, ignoring fields the selected type doesn't use.
    fn to_account(&self) -> Result<Account, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Please enter an account name.".into());
        }
        let balance = if self.balance.trim().is_empty() { 0.0 } else { parse_num(&self.balance).ok_or("Balance must be a number.")? };
        let sp = spec(self.kind);
        let num = |s: &str| parse_num(s).unwrap_or(0.0);
        let has_contrib = sp.contrib_field.is_some();
        Ok(Account {
            id: self.id,
            name: name.into(),
            kind: self.kind,
            balance,
            annual_return_pct: if sp.return_field.is_some() { num(&self.ret) } else { 0.0 },
            monthly_contribution: if has_contrib { num(&self.contrib).max(0.0) } else { 0.0 },
            contribution_from_cash: match sp.funding {
                Funding::FromChecking => true,
                Funding::Payroll => false,
                Funding::Choice(_) => self.from_cash,
            } && has_contrib,
            employer_match_monthly: if sp.match_label.is_some() { num(&self.matched).max(0.0) } else { 0.0 },
            notes: self.notes.trim().into(),
            purpose: self.purpose.trim().into(),
            minimum_balance: if self.minimum.trim().is_empty() { 0.0 } else { parse_num(&self.minimum).ok_or("Minimum balance must be a number.")?.max(0.0) },
        })
    }
}

#[derive(Clone, PartialEq)]
struct BucketForm {
    id: i64,
    name: String,
    balance: String,
    monthly_add: String,
    target: String,
}

impl BucketForm {
    fn new() -> Self {
        BucketForm { id: 0, name: String::new(), balance: String::new(), monthly_add: String::new(), target: String::new() }
    }
    fn from_bucket(b: &Bucket) -> Self {
        BucketForm {
            id: b.id,
            name: b.name.clone(),
            balance: num_str(b.balance),
            monthly_add: if b.monthly_add > 0.0 { num_str(b.monthly_add) } else { String::new() },
            target: b.target.map(num_str).unwrap_or_default(),
        }
    }
    fn to_bucket(&self, account_id: i64) -> Result<Bucket, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Give the bucket a name, e.g. Rainy day money.".into());
        }
        let num = |s: &str, what: &str| -> Result<f64, String> {
            if s.trim().is_empty() { Ok(0.0) } else { parse_num(s).map(|v| v.max(0.0)).ok_or(format!("{what} must be a number.")) }
        };
        let target = if self.target.trim().is_empty() { None } else { Some(num(&self.target, "Target")?) };
        Ok(Bucket {
            id: self.id,
            account_id,
            name: name.into(),
            balance: num(&self.balance, "Amount")?,
            monthly_add: num(&self.monthly_add, "Monthly top-up")?,
            target,
            notes: String::new(),
            debt_id: None,
        })
    }
}


#[component]
pub fn AccountsPage() -> impl IntoView {
    let store = use_store();
    let form = RwSignal::new(AccountForm::new());
    let groups = [(AccountGroup::Cash, "Cash"), (AccountGroup::Investments, "Investments"), (AccountGroup::Retirement, "Retirement")];

    let stats = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        view! {
            <div class="grid grid-cols-2 gap-4 @3xl:grid-cols-4">
                <Stat label="Net worth" value=money(data.net_worth(), &sym) />
                <Stat label="Cash" value=money(data.group_total(AccountGroup::Cash), &sym) />
                <Stat label="Investments" value=money(data.group_total(AccountGroup::Investments), &sym) />
                <Stat label="Retirement" value=money(data.group_total(AccountGroup::Retirement), &sym) />
            </div>
        }
    };
    let lists = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        let today = store.today();
        let empty = data.accounts.is_empty().then(|| view! {
            <EmptyState title="No accounts yet" text="Add your checking account first: projected leftover cash collects there." />
        });
        let cards = groups
            .into_iter()
            .filter(|(g, _)| data.accounts.iter().any(|a| a.kind.group() == *g))
            .map(|(g, gname)| {
                let total = money(data.group_total(g), &sym);
                let items = data
                    .accounts
                    .iter()
                    .filter(|a| a.kind.group() == g)
                    .map(|a| {
                        let flow = data.money_flow(a.id, today);
                        let received = (!flow.received.is_empty())
                            .then(|| format!("Deposits: {} · +{}/mo", flow.received.join(", "), money(flow.received_monthly, &sym)));
                        let paid = (!flow.paid.is_empty()).then(|| format!("Pays: {} · −{}/mo", flow.paid.join(", "), money(flow.paid_monthly, &sym)));
                        let holdings: Vec<Holding> = data.holdings_for(a.id).cloned().collect();
                        let buckets: Vec<Bucket> = data.buckets_for(a.id).cloned().collect();
                        view! {
                            <AccountItem
                                account=a.clone()
                                value=data.account_value(a)
                                holdings=holdings
                                buckets=buckets
                                received=received
                                paid=paid
                                sym=sym.clone()
                                form=form
                            />
                        }
                    })
                    .collect_view();
                view! {
                    <Card>
                        <CardHeader>
                            <CardTitle>{gname}</CardTitle>
                            <CardDescription>{total}</CardDescription>
                        </CardHeader>
                        <CardContent class="flex flex-col gap-4">{items}</CardContent>
                    </Card>
                }
            })
            .collect_view();
        view! { {empty} {cards} }
    };

    view! {
        <PageHeader title="Accounts & investments"
            subtitle="Checking, savings, brokerage and retirement accounts. Net income lands in your first checking account in projections." />
        {stats}
        <div class="grid gap-6 @5xl:grid-cols-3">
            <div class="flex flex-col gap-6 @5xl:col-span-2">{lists}</div>
            <AccountFormCard form=form />
        </div>
    }
}

/// The add/edit form. Its fields depend on the selected account type.
#[component]
fn AccountFormCard(form: RwSignal<AccountForm>) -> impl IntoView {
    let store = use_store();
    let error = RwSignal::new(None::<String>);
    let kind = Memo::new(move |_| form.with(|f| f.kind));
    let editing = move || form.with(|f| f.id != 0);
    let sym = move || store.currency();
    let kind_options: Vec<(String, String)> = AccountKind::ALL.iter().map(|k| (k.key().to_string(), k.label().to_string())).collect();
    let save = move |_| match form.with_untracked(AccountForm::to_account) {
        Ok(a) => {
            error.set(None);
            store.apply(Change::SaveAccount(a.clone()), if a.id == 0 { "Account added" } else { "Account saved" });
            form.set(AccountForm::new());
        }
        Err(e) => error.set(Some(e)),
    };

    // The fields that depend on the type, rebuilt only when the type changes.
    let type_fields = move || {
        let k = kind.get();
        let sp = spec(k);
        let market = store.snap.with_untracked(|s| s.as_ref().map(|s| s.market.clone()).unwrap_or_default());
        let ret = sp.return_field.map(|(label, hint)| view! {
            <Field label=label hint=return_hint(k, hint, &market)>
                <FormInput form=form get=|f: &AccountForm| f.ret.clone() set=|f: &mut AccountForm, v| f.ret = v inputmode="decimal" />
            </Field>
        });
        let contrib = sp.contrib_field.map(|(label, hint)| {
            let matched = sp.match_label.map(|m| view! {
                <Field label=format!("{m} ({})", sym())>
                    <FormInput form=form get=|f: &AccountForm| f.matched.clone() set=|f: &mut AccountForm, v| f.matched = v placeholder="0" inputmode="decimal" />
                </Field>
            });
            let choice = matches!(sp.funding, Funding::Choice(_)).then(|| view! {
                <SettingRow title="Paid from checking" caption="Off if it's deducted from your paycheck (your take-home pay already excludes it).">
                    <FormSwitch form=form get=|f: &AccountForm| f.from_cash set=|f: &mut AccountForm, v| f.from_cash = v label="Paid from checking" />
                </SettingRow>
            });
            view! {
                <div class=if sp.match_label.is_some() { "grid grid-cols-2 items-end gap-3" } else { "grid grid-cols-1 gap-3" }>
                    <Field label=format!("{label} ({})", sym())>
                        <FormInput form=form get=|f: &AccountForm| f.contrib.clone() set=|f: &mut AccountForm, v| f.contrib = v placeholder="0" inputmode="decimal" />
                    </Field>
                    {matched}
                </div>
                <p class="-mt-2 text-xs text-muted-foreground">{hint}</p>
                {choice}
            }
        });
        view! { {ret} {contrib} }
    };

    view! {
        <Card>
            <CardHeader>
                <CardTitle>{move || { let n = spec(kind.get()).noun; if editing() { format!("Edit {n}") } else { format!("Add {n}") } }}</CardTitle>
                <CardDescription>{move || spec(kind.get()).intro}</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-4">
                <Field label="Type">
                    <NativeSelect label="Account type" value=Signal::derive(move || kind.get().key().to_string()) options=kind_options
                        on_change=move |v: String| if let Some(k) = AccountKind::from_key(&v) {
                            form.update(|f| {
                                if f.id == 0 {
                                    // New account: take the type's defaults, keep what was typed.
                                    let keep = (f.name.clone(), f.balance.clone(), f.notes.clone(), f.purpose.clone(), f.minimum.clone());
                                    *f = AccountForm::blank(k);
                                    (f.name, f.balance, f.notes, f.purpose, f.minimum) = keep;
                                } else {
                                    f.kind = k;
                                }
                            })
                        } />
                </Field>
                {move || {
                    let placeholder = spec(kind.get()).name_placeholder;
                    view! {
                        <Field label="Name">
                            <FormInput form=form get=|f: &AccountForm| f.name.clone() set=|f: &mut AccountForm, v| f.name = v placeholder=placeholder />
                        </Field>
                    }
                }}
                {move || {
                    let sp = spec(kind.get());
                    let label = format!("{} ({})", sp.balance_label, sym());
                    let input = view! {
                        <FormInput form=form get=|f: &AccountForm| f.balance.clone() set=|f: &mut AccountForm, v| f.balance = v placeholder="0" inputmode="decimal" />
                    };
                    let hint = sp.balance_hint.map(|h| view! { <p class="text-xs text-muted-foreground">{h}</p> });
                    view! { <Field label=label>{input}{hint}</Field> }
                }}
                <Field label="Purpose (optional)" hint="What this account's money is set aside for.">
                    <FormInput form=form get=|f: &AccountForm| f.purpose.clone() set=|f: &mut AccountForm, v| f.purpose = v placeholder="e.g. Medical, Food, Emergency fund" />
                    <div class="flex flex-wrap gap-1">
                        {PURPOSES.iter().map(|c| view! {
                            <button type="button" class="cursor-pointer rounded-full border px-2 py-0.5 text-[0.6875rem] text-muted-foreground hover:bg-accent"
                                on:click=move |_| form.update(|f| f.purpose = c.to_string())>{*c}</button>
                        }).collect_view()}
                    </div>
                </Field>
                {move || {
                    let hint = if kind.get().group() == AccountGroup::Cash {
                        "Your operating baseline. Savings goals never spend below this."
                    } else {
                        "A floor to keep an eye on; the account is flagged if it drops below."
                    };
                    view! {
                        <Field label=format!("Minimum balance to keep ({})", sym()) hint=hint>
                            <FormInput form=form get=|f: &AccountForm| f.minimum.clone() set=|f: &mut AccountForm, v| f.minimum = v placeholder="0" inputmode="decimal" />
                        </Field>
                    }
                }}
                {type_fields}
                <Field label="Notes">
                    <FormInput form=form get=|f: &AccountForm| f.notes.clone() set=|f: &mut AccountForm, v| f.notes = v />
                </Field>
                {move || error.get().map(|e| view! { <p class="text-sm text-destructive">{e}</p> })}
                <div class="flex gap-2">
                    <Button on_click=save><Icon name=IconName::Plus />{move || if editing() { "Save changes".to_string() } else { format!("Add {}", spec(kind.get()).noun) }}</Button>
                    {move || editing().then(|| view! { <Button variant=ButtonVariant::Outline on_click=move |_| { error.set(None); form.set(AccountForm::new()) }>"Cancel"</Button> })}
                </div>
            </CardContent>
        </Card>
    }
}

#[component]
fn AccountItem(
    account: Account,
    value: f64,
    holdings: Vec<Holding>,
    buckets: Vec<Bucket>,
    received: Option<String>,
    paid: Option<String>,
    sym: String,
    form: RwSignal<AccountForm>,
) -> impl IntoView {
    let store = use_store();
    let id = account.id;
    let a2 = account.clone();
    let invests = account.kind.group() != AccountGroup::Cash;
    let has_tags = !account.purpose.is_empty() || account.minimum_balance > 0.0;
    let purpose_text = account.purpose.clone();
    let purpose = (!purpose_text.is_empty()).then(move || view! { <Badge variant=BadgeVariant::Info>{purpose_text}</Badge> });
    let minimum = (account.minimum_balance > 0.0).then(|| view! { <span class="text-muted-foreground">{format!("Keep at least {}", money(account.minimum_balance, &sym))}</span> });
    let below = (account.minimum_balance > 0.0 && value < account.minimum_balance)
        .then(|| view! { <span class="font-medium text-destructive">{format!("Below minimum by {}", money(account.minimum_balance - value, &sym))}</span> });
    let mut terms = format!("{}%/yr", num_str(account.annual_return_pct));
    if account.monthly_contribution > 0.0 {
        terms += &format!(" · +{}/mo{}", money(account.monthly_contribution, &sym), if account.contribution_from_cash { " from checking" } else { " via payroll" });
    }
    if account.employer_match_monthly > 0.0 {
        terms += &format!(" · +{}/mo match", money(account.employer_match_monthly, &sym));
    }
    let flows = (received.is_some() || paid.is_some()).then(|| view! {
        <div class="flex flex-col gap-0.5 text-xs text-muted-foreground">
            {received.map(|r| view! { <span>{r}</span> })}
            {paid.map(|p| view! { <span>{p}</span> })}
        </div>
    });
    let holdings_view = invests.then(|| view! { <Holdings account_id=id holdings=holdings sym=sym.clone() cash=account.balance /> });
    view! {
        <div class=move || if form.with(|f| f.id) == id { "rounded-lg border border-ring p-4 flex flex-col gap-3" } else { "rounded-lg border p-4 flex flex-col gap-3" }>
            <div class="flex items-start gap-3">
                <div class="flex flex-1 flex-col gap-1">
                    <div class="flex items-center gap-2">
                        <span class="font-medium">{account.name.clone()}</span>
                        <Badge>{account.kind.label()}</Badge>
                    </div>
                    {has_tags.then(|| view! { <div class="flex flex-wrap items-center gap-2 text-xs">{purpose}{minimum}{below}</div> })}
                    <div class="text-xs text-muted-foreground">{terms}</div>
                </div>
                <div class="text-right text-lg font-semibold tabular-nums">{money(value, &sym)}</div>
                <div class="flex whitespace-nowrap">
                    <RowActions on_edit=move |_| form.set(AccountForm::from_account(&a2)) on_delete=move |_| {
                        store.apply(Change::DeleteAccount(id), "Account deleted");
                        if form.with_untracked(|f| f.id == id) { form.set(AccountForm::new()) }
                    } />
                </div>
            </div>
            {flows}
            <BucketsPanel account_id=id value=value buckets=buckets sym=sym.clone() />
            {holdings_view}
        </div>
    }
}

/// Buckets inside an account: earmarked portions such as "Rainy day money".
#[component]
fn BucketsPanel(account_id: i64, value: f64, buckets: Vec<Bucket>, sym: String) -> impl IntoView {
    let store = use_store();
    let editing = RwSignal::new(None::<BucketForm>);
    let error = RwSignal::new(None::<String>);
    let allocated: f64 = buckets.iter().map(|b| b.balance).sum();
    let unallocated = value - allocated;
    let save = move |_| {
        let Some(f) = editing.get_untracked() else { return };
        match f.to_bucket(account_id) {
            Ok(b) => {
                error.set(None);
                store.apply(Change::SaveBucket(b.clone()), if b.id == 0 { "Bucket added" } else { "Bucket saved" });
                editing.set(None);
            }
            Err(e) => error.set(Some(e)),
        }
    };
    let none = buckets.is_empty();
    let rows = buckets
        .into_iter()
        .map(|b| {
            let id = b.id;
            let is_debt = b.debt_id.is_some();
            let b2 = b.clone();
            let mut details = Vec::new();
            if is_debt {
                details.push("This month's planned payment · follows your debt payoff plan".to_string());
            }
            if b.monthly_add > 0.0 {
                details.push(format!("+{}/mo", money(b.monthly_add, &sym)));
            }
            if let Some(t) = b.target {
                details.push(format!("target {}", money(t, &sym)));
            }
            let progress = b.target.filter(|t| *t > 0.0).map(|t| view! { <Progress value=b.balance.min(t) max=t class="h-1.5" /> });
            let delete_title = if is_debt { "Stop setting aside this debt's payments (the payoff plan continues)" } else { "Delete bucket (its bills fall back to this account)" };
            view! {
                <div class="flex flex-col gap-1 rounded-md border bg-background px-3 py-2">
                    <div class="flex items-center gap-2">
                        <span class="flex-1 text-sm font-medium">{b.name.clone()}</span>
                        {is_debt.then(|| view! { <Badge variant=BadgeVariant::Warning>"Debt payment"</Badge> })}
                        <span class="text-sm font-semibold tabular-nums">{money(b.balance, &sym)}</span>
                        {(!is_debt).then(|| view! {
                            <Button size=ButtonSize::IconXs variant=ButtonVariant::Ghost title="Edit bucket" label="Edit bucket"
                                on_click=move |_| { error.set(None); editing.set(Some(BucketForm::from_bucket(&b2))) }>
                                <Icon name=IconName::Pencil class="size-3.5" />
                            </Button>
                        })}
                        <Button size=ButtonSize::IconXs variant=ButtonVariant::Ghost title=delete_title label=delete_title
                            on_click=move |_| store.apply(Change::DeleteBucket(id), "Bucket deleted")>
                            <Icon name=IconName::Trash class="size-3.5" />
                        </Button>
                    </div>
                    <div class="flex flex-wrap gap-x-3 text-xs text-muted-foreground">
                        {details.into_iter().map(|d| view! { <span>{d}</span> }).collect_view()}
                    </div>
                    {progress}
                </div>
            }
        })
        .collect_view();
    let sym2 = sym.clone();
    let editor = move || match editing.get() {
        Some(f) => view! {
            <div class="flex flex-col gap-2 rounded-md border bg-background p-3">
                <div class="grid grid-cols-2 gap-2">
                    <Field label="Bucket name">
                        <BucketInput editing=editing get=|f: &BucketForm| f.name.clone() set=|f: &mut BucketForm, v| f.name = v placeholder="e.g. Rainy day money" />
                    </Field>
                    <Field label=format!("Set aside now ({sym2})")>
                        <BucketInput editing=editing get=|f: &BucketForm| f.balance.clone() set=|f: &mut BucketForm, v| f.balance = v placeholder="0" />
                    </Field>
                    <Field label=format!("Add each month ({sym2})")>
                        <BucketInput editing=editing get=|f: &BucketForm| f.monthly_add.clone() set=|f: &mut BucketForm, v| f.monthly_add = v placeholder="0" />
                    </Field>
                    <Field label="Target (optional)">
                        <BucketInput editing=editing get=|f: &BucketForm| f.target.clone() set=|f: &mut BucketForm, v| f.target = v placeholder="No limit" />
                    </Field>
                </div>
                <p class="text-xs text-muted-foreground">
                    "Money set aside here stays in this account but is reserved: savings goals won't spend it. Monthly top-ups stop at the target. Choose this bucket as \u{201c}Paid from\u{201d} on a bill or expense to pay it from here."
                </p>
                {move || error.get().map(|e| view! { <p class="text-xs text-destructive">{e}</p> })}
                <div class="flex gap-2">
                    <Button size=ButtonSize::Sm on_click=save>{if f.id == 0 { "Add bucket" } else { "Save bucket" }}</Button>
                    <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost on_click=move |_| { error.set(None); editing.set(None) }>"Cancel"</Button>
                </div>
            </div>
        }
        .into_any(),
        None => view! {
            <div>
                <Button variant=ButtonVariant::Link size=ButtonSize::Sm class="px-0" on_click=move |_| { error.set(None); editing.set(Some(BucketForm::new())) }>
                    {if none { "+ Split into buckets (e.g. Rainy day money)" } else { "+ Add bucket" }}
                </Button>
            </div>
        }
        .into_any(),
    };
    let header = (!none).then(|| {
        let (class, text) = if unallocated < -0.005 {
            ("font-medium text-destructive", format!("Buckets are {} more than the balance", money(-unallocated, &sym)))
        } else {
            ("text-muted-foreground", format!("Unallocated: {}", money(unallocated, &sym)))
        };
        view! {
            <div class="flex flex-wrap items-center justify-between gap-2 text-xs">
                <span class="font-medium text-foreground">"Buckets"</span>
                <span class=class>{text}</span>
            </div>
        }
    });
    view! {
        <div class=move || if none && editing.with(Option::is_none) { "" } else { "flex flex-col gap-2 rounded-md bg-muted/50 p-3" }>
            {header}
            {rows}
            {editor}
        </div>
    }
}

/// A text input bound to a field of the bucket being edited.
#[component]
fn BucketInput<G, S>(editing: RwSignal<Option<BucketForm>>, get: G, set: S, #[prop(into)] placeholder: String) -> impl IntoView
where
    G: Fn(&BucketForm) -> String + Send + Sync + 'static,
    S: Fn(&mut BucketForm, String) + Send + Sync + 'static,
{
    view! {
        <input type="text" class=crate::ui::INPUT_CLASS placeholder=placeholder
            prop:value=move || editing.with(|f| f.as_ref().map(&get).unwrap_or_default())
            on:input=move |ev| { let v = event_target_value(&ev); editing.update(|f| if let Some(f) = f { set(f, v) }) } />
    }
}

#[component]
fn Holdings(account_id: i64, holdings: Vec<Holding>, sym: String, cash: f64) -> impl IntoView {
    let open = RwSignal::new(!holdings.is_empty());
    let holdings = StoredValue::new(holdings);
    let sym = StoredValue::new(sym);
    view! {
        <Show
            when=move || open.get()
            fallback=move || view! {
                <div>
                    <Button variant=ButtonVariant::Link size=ButtonSize::Sm class="px-0" on_click=move |_| open.set(true)>
                        "+ Track individual holdings (stocks, funds)"
                    </Button>
                </div>
            }
        >
            <HoldingsBody account_id=account_id holdings=holdings.get_value() sym=sym.get_value() cash=cash />
        </Show>
    }
}

#[component]
fn HoldingsBody(account_id: i64, holdings: Vec<Holding>, sym: String, cash: f64) -> impl IntoView {
    let store = use_store();
    let symbol = RwSignal::new(String::new());
    let shares = RwSignal::new(String::new());
    let price = RwSignal::new(String::new());
    let add = move |_| {
        let s = symbol.get_untracked().trim().to_uppercase();
        match (parse_num(&shares.get_untracked()), parse_num(&price.get_untracked())) {
            (Some(sh), Some(p)) if !s.is_empty() => {
                let h = Holding { id: 0, account_id, symbol: s, shares: sh, price: p, price_updated: Some(store.today()) };
                store.apply(Change::SaveHolding(h), "Holding added");
                symbol.set(String::new());
                shares.set(String::new());
                price.set(String::new());
            }
            _ => store.notify("Enter a symbol, number of shares and price per share.", true),
        }
    };
    let has_rows = !holdings.is_empty();
    let rows = holdings.into_iter().map(|h| view! { <HoldingRow holding=h sym=sym.clone() /> }).collect_view();
    let cash_row = (cash.abs() > 0.005).then(|| view! {
        <TableRow>
            <TableCell class="text-muted-foreground">"Cash / other"</TableCell>
            <TableCell>""</TableCell>
            <TableCell>""</TableCell>
            <TableCell class="text-right tabular-nums">{money_cents(cash, &sym)}</TableCell>
            <TableCell>""</TableCell>
        </TableRow>
    });
    let table = has_rows.then(|| view! {
        <Table>
            <thead class="[&_tr]:border-b">
                <TableRow>
                    <TableHead>"Symbol"</TableHead>
                    <TableHead class="text-right">"Shares"</TableHead>
                    <TableHead class="text-right">"Price"</TableHead>
                    <TableHead class="text-right">"Value"</TableHead>
                    <TableHead>""</TableHead>
                </TableRow>
            </thead>
            <TableBody>{rows}{cash_row}</TableBody>
        </Table>
    });
    view! {
        <div class="flex flex-col gap-2 rounded-md bg-muted/50 p-3">
            <div class="flex items-center justify-between text-xs text-muted-foreground">
                <span>"Holdings · prices entered manually (offline). Live quotes are planned for a later version."</span>
            </div>
            {table}
            <div class="grid grid-cols-[1fr_1fr_1fr_auto] gap-2">
                <Input value=symbol placeholder="Symbol" />
                <Input value=shares placeholder="Shares" />
                <Input value=price placeholder="Price/share" />
                <Button variant=ButtonVariant::Secondary on_click=add><Icon name=IconName::Plus />"Add"</Button>
            </div>
        </div>
    }
}

#[component]
fn HoldingRow(holding: Holding, sym: String) -> impl IntoView {
    let store = use_store();
    let price = RwSignal::new(num_str(holding.price));
    let shares = RwSignal::new(num_str(holding.shares));
    let (orig_price, orig_shares) = (num_str(holding.price), num_str(holding.shares));
    let dirty = move || price.get() != orig_price || shares.get() != orig_shares;
    let id = holding.id;
    let h = holding.clone();
    let updated = holding.price_updated.map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_default();
    let (symbol, value) = (holding.symbol.clone(), money_cents(holding.value(), &sym));
    let small = "text-foreground placeholder:text-muted-foreground dark:bg-input/30 border-input flex h-7 w-24 ml-auto min-w-0 rounded-md border bg-transparent px-3 py-1 text-right text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-2";
    view! {
        <TableRow>
            <TableCell class="font-medium">
                {symbol}
                {(!updated.is_empty()).then(|| view! { <div class="text-[0.625rem] font-normal text-muted-foreground">{format!("price {updated}")}</div> })}
            </TableCell>
            <TableCell class="text-right">
                <input type="text" inputmode="decimal" class=small aria-label="Shares" prop:value=move || shares.get() on:input=move |ev| shares.set(event_target_value(&ev)) />
            </TableCell>
            <TableCell class="text-right">
                <input type="text" inputmode="decimal" class=small aria-label="Price" prop:value=move || price.get() on:input=move |ev| price.set(event_target_value(&ev)) />
            </TableCell>
            <TableCell class="text-right tabular-nums">{value}</TableCell>
            <TableCell class="text-right whitespace-nowrap">
                {move || {
                    let h = h.clone();
                    dirty().then(move || view! {
                        <Button size=ButtonSize::Sm on_click=move |_| match (parse_num(&shares.get_untracked()), parse_num(&price.get_untracked())) {
                            (Some(s), Some(p)) => store.apply(Change::SaveHolding(Holding { shares: s, price: p, price_updated: Some(store.today()), ..h.clone() }), "Holding updated"),
                            _ => store.notify("Shares and price must be numbers.", true),
                        }>"Save"</Button>
                    })
                }}
                <Button size=ButtonSize::IconSm variant=ButtonVariant::Ghost title="Remove holding" label="Remove holding"
                    on_click=move |_| store.apply(Change::DeleteHolding(id), "Holding removed")>
                    <Icon name=IconName::Trash />
                </Button>
            </TableCell>
        </TableRow>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(kind: AccountKind) -> AccountForm {
        AccountForm {
            name: "x".into(),
            balance: "1000".into(),
            ret: "5".into(),
            contrib: "200".into(),
            matched: "50".into(),
            from_cash: true,
            ..AccountForm::blank(kind)
        }
    }

    #[test]
    fn fields_follow_account_type() {
        let a = form(AccountKind::Checking).to_account().unwrap();
        assert_eq!((a.annual_return_pct, a.monthly_contribution, a.employer_match_monthly), (0.0, 0.0, 0.0));
        assert!(!a.contribution_from_cash);

        let a = form(AccountKind::Savings).to_account().unwrap();
        assert_eq!((a.annual_return_pct, a.monthly_contribution, a.employer_match_monthly), (5.0, 200.0, 0.0));
        assert!(a.contribution_from_cash);

        // 401(k): always payroll, keeps the match even if the switch state says otherwise.
        let a = form(AccountKind::Retirement401k).to_account().unwrap();
        assert_eq!((a.monthly_contribution, a.employer_match_monthly), (200.0, 50.0));
        assert!(!a.contribution_from_cash);

        // IRA: user's choice, no match.
        let mut f = form(AccountKind::RothIra);
        f.from_cash = false;
        let a = f.to_account().unwrap();
        assert_eq!(a.employer_match_monthly, 0.0);
        assert!(!a.contribution_from_cash);

        let a = form(AccountKind::Other).to_account().unwrap();
        assert_eq!((a.annual_return_pct, a.monthly_contribution), (5.0, 0.0));
    }

    #[test]
    fn defaults_per_type() {
        assert!(AccountForm::blank(AccountKind::RothIra).from_cash);
        assert!(!AccountForm::blank(AccountKind::Hsa).from_cash);
        assert!(!AccountForm::blank(AccountKind::Retirement401k).from_cash);
        assert_eq!(AccountForm::blank(AccountKind::Savings).ret, "4");
    }
}
