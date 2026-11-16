# Debt payoff

Add credit cards and loans to see what they cost each month and get a plan to pay them off **while
still covering rent, utilities and your other bills**.

![Debt payoff](https://raw.githubusercontent.com/tobeoffline64/PenguinFin/main/docs/screenshots/debt-payoff.png)

## Adding a debt

From your latest statement: **balance**, **interest rate (APR)** and **minimum payment**, plus the
account the payments come from (default: your first checking account). Types: credit card, personal
loan, student loan, auto loan, medical bill, other.

While you type, the form shows the interest charged this month and how long paying only the minimum
would take. A warning appears if the minimum doesn't even cover the interest.

## Payment buckets (automatic)

With **Set aside payments in a bucket** on (the default for new debts), PenguinFin adds a
*"Visa card payment"* bucket to the account that pays the debt. The bucket always holds that debt's
**planned payment for this month** (its minimum, plus the extra when it's the debt being targeted) and
updates itself whenever balances, the extra amount or the strategy change, and at the start of each month.

- The money is reserved: savings goals and spendable cash don't count it.
- In projections the bucket follows the plan and empties once the debt is paid off.
- Delete the debt and its bucket goes too. Remove just the bucket (or turn the switch off) to stop
  setting money aside. The payoff plan continues either way.
- For debts added before this feature, use **Set aside payments in a bucket** on the debt.

## Your monthly budget

The budget card starts from your take-home income and subtracts bills (rent, utilities, insurance…),
recurring expenses and savings transfers. What's left is **available for debt**. The plan then takes the
minimum payments and the **extra** you choose, and shows what's left over. You're warned if:

- the minimums alone are more than what's left after bills, or
- the plan needs more than you have. It tells you the most you can add without missing a bill.

**Use $…** sets the extra to everything that's left.

## Strategy

- **Avalanche**: extra money goes to the highest interest rate first. Pays the least interest.
- **Snowball**: extra money goes to the smallest balance first. Quick wins.

When a debt is paid off, its payment rolls into the next one, so your monthly debt payment stays the
same until everything is paid.

## The plan

![Payoff plan](https://raw.githubusercontent.com/tobeoffline64/PenguinFin/main/docs/screenshots/debt-payoff-plan.png)

- **Debt-free date**, total interest, and how much you save compared with paying only minimums.
- A **chart** of each balance shrinking over time.
- Each debt, in payoff order: payoff date and interest with your plan vs. minimums only, and the monthly
  payment needed to be debt-free in 1, 2 or 3 years (standard loan amortization).
- A month-by-month **payment schedule** (✓ marks the month a debt is paid off).

## One-time payments

Paid something extra, like a tax refund or a bonus? Click **Log a payment** on that debt:

- **Amount**, **Date** (today by default), **Paid from** (the debt's usual account, or any other) and an
  optional **Note**.
- **Today or earlier**: the payment is logged. The debt's balance drops by the amount (never below zero)
  and, with **Also subtract it from … balance** on, the account's balance drops too. Turn that switch off
  if you've already updated the account balance yourself.
- **A future date**: the payment is **scheduled**. It shows as *Planned* on the debt, and the payoff plan,
  schedule, dashboard and projections already include it (taken from the chosen account in that month).
  After the date passes it shows *Was planned … Did you make it?* with **Log it** (applies it as above) or
  **Remove**.

The debt card lists its planned and latest payments, and the **Payment history** card at the bottom of the
page lists all of them. **Undo** puts both the debt balance and the account balance back exactly as they
were. Payments are kept in JSON backups; restoring a backup doesn't apply them a second time.

## Everywhere else

- The **Dashboard** overview chart shows a red **Debt** line and your debt-free date, and a **Debt payoff**
  card below it shows what you owe, your monthly payment, total interest and each balance month by month.

- **Net worth** subtracts what you owe (dashboard, accounts, projections).
- **Projections** pay debts from their accounts each month according to your plan, and show a Debt line
  and column. Savings goals and cash forecasts include the payments.
- The dashboard's **Left over / month** is after your debt plan.

Interest is calculated monthly (APR ÷ 12). Update balances from your statements now and then; the plan
recalculates from there.
