//! A first dashboard: the headline numbers, so an upgraded or imported file can be checked
//! against what PenguinFin 1 showed. The full dashboard is ported in phase 3.

use crate::app::{Page, use_store};
use crate::ui::*;
use leptos::prelude::*;
use penguinfin_core::format::money;
use penguinfin_core::model::AccountGroup;

#[component]
pub fn Overview(#[prop(into)] on_nav: Callback<Page>) -> impl IntoView {
    let store = use_store();
    let sym = move || store.currency();
    let data = move || store.snap.with(|s| s.as_ref().map(|s| s.data.clone()).unwrap_or_default());
    let dismiss = move |_| store.upgraded.set(None);
    view! {
        <PageHeader title="Dashboard" subtitle="PenguinFin 2 preview: more of this page arrives as each part moves over." />
        {move || {
            store
                .upgraded
                .get()
                .map(|copy| {
                    view! {
                        <div class="flex flex-col gap-2 rounded-lg border bg-muted/40 p-4 text-sm">
                            <div class="font-medium">"Your PenguinFin 1 data was upgraded"</div>
                            <p class="text-muted-foreground">"Check the numbers below against PenguinFin 1. A copy of your data from before the upgrade was saved at:"</p>
                            <code class="rounded bg-muted px-2 py-1 text-xs break-all">{copy}</code>
                            <div><Button size=ButtonSize::Sm variant=ButtonVariant::Outline on_click=dismiss>"Got it"</Button></div>
                        </div>
                    }
                })
        }}
        <div class="grid grid-cols-2 gap-4 @3xl:grid-cols-4">
            {move || {
                let d = data();
                let s = sym();
                let cash = d.group_total(AccountGroup::Cash);
                let invest = d.group_total(AccountGroup::Investments);
                let retire = d.group_total(AccountGroup::Retirement);
                let debt_tone = if d.total_debt() > 0.0 { "bad" } else { "" };
                view! {
                    <Stat label="Net worth" value=money(d.net_worth(), &s) sub=format!("Cash {} · Invest {} · Retire {}", money(cash, &s), money(invest, &s), money(retire, &s)) />
                    <Stat label="Total debt" value=money(d.total_debt(), &s) sub=format!("{} debts", d.debts.len()) tone=debt_tone />
                    <Stat label="Accounts" value=d.accounts.len().to_string() sub=format!("{} buckets · {} holdings", d.buckets.len(), d.holdings.len()) />
                    <Stat label="Income, bills, expenses" value=d.flows.len().to_string() sub=format!("{} goals · {} debt payments", d.goals.len(), d.debt_payments.len()) />
                }
            }}
        </div>
        <Card>
            <CardHeader>
                <CardTitle>"Moving to PenguinFin 2"</CardTitle>
                <CardDescription>"Settings & data is ready. The other pages are being moved one at a time; until then, use PenguinFin 1 for them."</CardDescription>
            </CardHeader>
            <CardContent>
                <Button variant=ButtonVariant::Outline on_click=move |_| on_nav.run(Page::Settings)>"Open Settings & data"</Button>
            </CardContent>
        </Card>
    }
}
