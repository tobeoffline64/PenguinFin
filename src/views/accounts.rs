use crate::app::{today, use_store};
use crate::ui::badge::*;
use crate::ui::button::*;
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::switch::Switch;
use crate::ui::table::*;
use crate::views::dashboard::PageHeader;
use crate::widgets::*;
use dioxus::prelude::*;
use penguinfin::model::{Account, AccountGroup, AccountKind, Holding};

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
            from_cash: !matches!(kind, AccountKind::Retirement401k | AccountKind::Pension),
            matched: "0".into(),
            notes: String::new(),
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
        }
    }
    fn to_account(&self) -> Result<Account, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Please enter an account name.".into());
        }
        let balance = if self.balance.trim().is_empty() { 0.0 } else { parse_num(&self.balance).ok_or("Balance must be a number.")? };
        Ok(Account {
            id: self.id,
            name: name.into(),
            kind: self.kind,
            balance,
            annual_return_pct: parse_num(&self.ret).unwrap_or(0.0),
            monthly_contribution: parse_num(&self.contrib).unwrap_or(0.0).max(0.0),
            contribution_from_cash: self.from_cash,
            employer_match_monthly: parse_num(&self.matched).unwrap_or(0.0).max(0.0),
            notes: self.notes.trim().into(),
        })
    }
}

#[component]
pub fn AccountsPage() -> Element {
    let mut store = use_store();
    let data = store.data.read().clone();
    let sym = data.settings.currency_symbol.clone();
    let mut form = use_signal(AccountForm::new);
    let mut error = use_signal(|| None::<String>);
    let mut generation = use_signal(|| 0u32);
    let editing = form.read().id != 0;
    let kind_options: Vec<(String, String)> =
        AccountKind::ALL.iter().map(|k| (k.key().to_string(), k.label().to_string())).collect();

    let save = move |_| {
        let res = form.read().to_account();
        match res {
        Ok(a) => {
            error.set(None);
            store.mutate(if a.id == 0 { "Account added" } else { "Account saved" }, |db| db.save_account(&a).map(|_| ()));
            form.set(AccountForm::new());
            generation += 1;
        }
        Err(e) => error.set(Some(e)),
    }};

    let groups = [
        (AccountGroup::Cash, "Cash"),
        (AccountGroup::Investments, "Investments"),
        (AccountGroup::Retirement, "Retirement"),
    ];

    rsx! {
        PageHeader { title: "Accounts & investments",
            subtitle: "Checking, savings, brokerage and retirement accounts. Net income lands in your first checking account in projections." }

        div { class: "grid grid-cols-2 gap-4 @3xl:grid-cols-4",
            Stat { label: "Net worth", value: money(data.net_worth(), &sym) }
            Stat { label: "Cash", value: money(data.group_total(AccountGroup::Cash), &sym) }
            Stat { label: "Investments", value: money(data.group_total(AccountGroup::Investments), &sym) }
            Stat { label: "Retirement", value: money(data.group_total(AccountGroup::Retirement), &sym) }
        }

        div { class: "grid gap-6 @5xl:grid-cols-3",
            div { class: "flex flex-col gap-6 @5xl:col-span-2",
                if data.accounts.is_empty() {
                    EmptyState { title: "No accounts yet", text: "Add your checking account first: projected leftover cash collects there." }
                }
                for (g, gname) in groups {
                    if data.accounts.iter().any(|a| a.kind.group() == g) {
                        Card { key: "{gname}",
                            CardHeader {
                                CardTitle { "{gname}" }
                                CardDescription { {money(data.group_total(g), &sym)} }
                            }
                            CardContent { class: "flex flex-col gap-4",
                                for a in data.accounts.iter().filter(|a| a.kind.group() == g).cloned() {
                                    AccountItem {
                                        key: "{a.id}",
                                        account: a.clone(),
                                        value: data.account_value(&a),
                                        holdings: data.holdings_for(a.id).cloned().collect::<Vec<_>>(),
                                        sym: sym.clone(),
                                        selected: form.read().id == a.id,
                                        on_edit: move |a: Account| { error.set(None); form.set(AccountForm::from_account(&a)); generation += 1; },
                                        on_delete: move |id: i64| {
                                            store.mutate("Account deleted", |db| db.delete_account(id));
                                            if form.read().id == id { form.set(AccountForm::new()); generation += 1; }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Card {
                CardHeader { CardTitle { if editing { "Edit account" } else { "Add account" } } }
                for g in [generation()] {
                CardContent { key: "{g}", class: "flex flex-col gap-4",
                    Field { label: "Type",
                        NativeSelect {
                            value: form.read().kind.key().to_string(),
                            options: kind_options.clone(),
                            onchange: move |v: String| {
                                if let Some(k) = AccountKind::from_key(&v) {
                                    let mut f = form.write();
                                    if f.id == 0 {
                                        // Fresh form: adopt the type's defaults but keep typed values.
                                        let keep = (f.name.clone(), f.balance.clone());
                                        *f = AccountForm::blank(k);
                                        (f.name, f.balance) = keep;
                                    } else {
                                        f.kind = k;
                                    }
                                    drop(f);
                                    generation += 1;
                                }
                            },
                        }
                    }
                    Field { label: "Name",
                        Input { value: form.read().name.clone(), placeholder: "e.g. Chase checking",
                            oninput: move |e: FormEvent| form.write().name = e.value() }
                    }
                    Field { label: "Current balance ({sym})", hint: "For brokerage/IRA: cash portion, or the total if you don't list holdings.",
                        Input { value: form.read().balance.clone(), placeholder: "0",
                            oninput: move |e: FormEvent| form.write().balance = e.value() }
                    }
                    Field { label: "Expected yearly return / APY %",
                        hint: "Long-run stock market average is often assumed at 6–8%; savings use APY.",
                        Input { value: form.read().ret.clone(),
                            oninput: move |e: FormEvent| form.write().ret = e.value() }
                    }
                    div { class: "grid grid-cols-2 gap-3",
                        Field { label: "Your monthly contribution",
                            Input { value: form.read().contrib.clone(),
                                oninput: move |e: FormEvent| form.write().contrib = e.value() }
                        }
                        Field { label: "Employer match / month",
                            Input { value: form.read().matched.clone(),
                                oninput: move |e: FormEvent| form.write().matched = e.value() }
                        }
                    }
                    div { class: "flex items-start justify-between gap-3",
                        div { class: "flex flex-col",
                            span { class: "text-sm", "Contribution comes out of checking" }
                            span { class: "text-xs text-muted-foreground",
                                "Turn off for payroll deductions like a 401(k): your entered take-home pay already excludes them." }
                        }
                        Switch { checked: form.read().from_cash, aria_label: "Contribution from checking",
                            onchange: move |v| form.write().from_cash = v }
                    }
                    Field { label: "Notes",
                        Input { value: form.read().notes.clone(), oninput: move |e: FormEvent| form.write().notes = e.value() }
                    }
                    if let Some(e) = error() {
                        p { class: "text-sm text-destructive", "{e}" }
                    }
                    div { class: "flex gap-2",
                        Button { onclick: save, Icon { name: IconName::Plus } if editing { "Save changes" } else { "Add account" } }
                        if editing {
                            Button { variant: ButtonVariant::Outline, onclick: move |_| { form.set(AccountForm::new()); generation += 1; }, "Cancel" }
                        }
                    }
                }
                }
            }
        }
    }
}

#[component]
fn AccountItem(
    account: Account,
    value: f64,
    holdings: Vec<Holding>,
    sym: String,
    selected: bool,
    on_edit: EventHandler<Account>,
    on_delete: EventHandler<i64>,
) -> Element {
    let mut confirm = use_signal(|| false);
    let id = account.id;
    let a2 = account.clone();
    let invests = account.kind.group() != AccountGroup::Cash;
    let border = if selected { "rounded-lg border border-ring p-4" } else { "rounded-lg border p-4" };
    rsx! {
        div { class: "{border} flex flex-col gap-3",
            div { class: "flex items-start gap-3",
                div { class: "flex flex-1 flex-col gap-1",
                    div { class: "flex items-center gap-2",
                        span { class: "font-medium", "{account.name}" }
                        Badge { variant: BadgeVariant::Secondary, size: BadgeSize::Sm, "{account.kind.label()}" }
                    }
                    div { class: "text-xs text-muted-foreground",
                        "{num_str(account.annual_return_pct)}%/yr"
                        if account.monthly_contribution > 0.0 {
                            " · +{money(account.monthly_contribution, &sym)}/mo"
                            if account.contribution_from_cash { " from checking" } else { " via payroll" }
                        }
                        if account.employer_match_monthly > 0.0 { " · +{money(account.employer_match_monthly, &sym)}/mo match" }
                    }
                }
                div { class: "text-right text-lg font-semibold tabular-nums", {money(value, &sym)} }
                div { class: "flex whitespace-nowrap",
                    if confirm() {
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Destructive, onclick: move |_| on_delete.call(id), "Delete" }
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| confirm.set(false), "Keep" }
                    } else {
                        Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Edit",
                            onclick: move |_| on_edit.call(a2.clone()), Icon { name: IconName::Pencil } }
                        Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Delete account and its holdings",
                            onclick: move |_| confirm.set(true), Icon { name: IconName::Trash } }
                    }
                }
            }
            if invests {
                Holdings { account_id: id, holdings, sym, cash: account.balance }
            }
        }
    }
}

#[component]
fn Holdings(account_id: i64, holdings: Vec<Holding>, sym: String, cash: f64) -> Element {
    let mut store = use_store();
    let mut symbol = use_signal(String::new);
    let mut shares = use_signal(String::new);
    let mut price = use_signal(String::new);
    let mut open = use_signal(|| !holdings.is_empty());
    let mut generation = use_signal(|| 0u32);

    let add = move |_| {
        let s = symbol.read().trim().to_uppercase();
        let parsed = (parse_num(&shares.read()), parse_num(&price.read()));
        match parsed {
            (Some(sh), Some(p)) if !s.is_empty() => {
                let h = Holding { id: 0, account_id, symbol: s, shares: sh, price: p, price_updated: Some(today()) };
                store.mutate("Holding added", |db| db.save_holding(&h).map(|_| ()));
                symbol.set(String::new());
                shares.set(String::new());
                price.set(String::new());
                generation += 1;
            }
            _ => store.notify("Enter a symbol, number of shares and price per share.", true),
        }
    };

    if !open() {
        return rsx! {
            div {
                Button { variant: ButtonVariant::Link, size: ButtonSize::Sm, class: "px-0",
                    onclick: move |_| open.set(true), "+ Track individual holdings (stocks, funds)" }
            }
        };
    }

    rsx! {
        div { class: "flex flex-col gap-2 rounded-md bg-muted/50 p-3",
            div { class: "flex items-center justify-between text-xs text-muted-foreground",
                span { "Holdings · prices entered manually (offline). Live quotes are planned for a later version." }
            }
            if !holdings.is_empty() {
                Table {
                    TableHeader { class: "bg-transparent",
                        TableRow {
                            TableHead { "Symbol" }
                            TableHead { class: "text-right", "Shares" }
                            TableHead { class: "text-right", "Price" }
                            TableHead { class: "text-right", "Value" }
                            TableHead { "" }
                        }
                    }
                    TableBody {
                        for h in holdings.iter().cloned() {
                            HoldingRow { key: "{h.id}", holding: h, sym: sym.clone() }
                        }
                        if cash.abs() > 0.005 {
                            TableRow {
                                TableCell { class: "text-muted-foreground", "Cash / other" }
                                TableCell { "" }
                                TableCell { "" }
                                TableCell { class: "text-right tabular-nums", {money_cents(cash, &sym)} }
                                TableCell { "" }
                            }
                        }
                    }
                }
            }
            for g in [generation()] {
            div { key: "{g}", class: "grid grid-cols-[1fr_1fr_1fr_auto] gap-2",
                Input { placeholder: "Symbol", value: symbol(), oninput: move |e: FormEvent| symbol.set(e.value()) }
                Input { placeholder: "Shares", value: shares(), oninput: move |e: FormEvent| shares.set(e.value()) }
                Input { placeholder: "Price/share", value: price(), oninput: move |e: FormEvent| price.set(e.value()) }
                Button { variant: ButtonVariant::Secondary, onclick: add, Icon { name: IconName::Plus } "Add" }
            }
            }
        }
    }
}

#[component]
fn HoldingRow(holding: Holding, sym: String) -> Element {
    let mut store = use_store();
    let mut price = use_signal(|| num_str(holding.price));
    let mut shares = use_signal(|| num_str(holding.shares));
    let h = holding.clone();
    let id = holding.id;
    let dirty = price() != num_str(holding.price) || shares() != num_str(holding.shares);
    let updated = holding.price_updated.map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_default();
    rsx! {
        TableRow {
            TableCell { class: "font-medium",
                "{holding.symbol}"
                if !updated.is_empty() { div { class: "text-[0.625rem] font-normal text-muted-foreground", "price {updated}" } }
            }
            TableCell { class: "text-right",
                Input { class: "h-7 w-24 ml-auto text-right", value: shares(), oninput: move |e: FormEvent| shares.set(e.value()) }
            }
            TableCell { class: "text-right",
                Input { class: "h-7 w-24 ml-auto text-right", value: price(), oninput: move |e: FormEvent| price.set(e.value()) }
            }
            TableCell { class: "text-right tabular-nums", {money_cents(holding.value(), &sym)} }
            TableCell { class: "text-right whitespace-nowrap",
                if dirty {
                    Button { size: ButtonSize::Sm, onclick: move |_| {
                        match (parse_num(&shares.read()), parse_num(&price.read())) {
                            (Some(s), Some(p)) => {
                                let upd = Holding { shares: s, price: p, price_updated: Some(today()), ..h.clone() };
                                store.mutate("Holding updated", |db| db.save_holding(&upd).map(|_| ()));
                            }
                            _ => store.notify("Shares and price must be numbers.", true),
                        }
                    }, "Save" }
                }
                Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Remove holding",
                    onclick: move |_| store.mutate("Holding removed", |db| db.delete_holding(id)), Icon { name: IconName::Trash } }
            }
        }
    }
}
