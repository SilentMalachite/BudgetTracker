use crate::domain::ledger::{Transaction, TxType};

/// Compute one account's current balance from a slice of transactions.
///
/// The slice may contain transactions for any account; this function picks
/// only the rows touching `account_id` (either as source or as the
/// destination of a transfer).
///
/// The rules (CLAUDE.md rule 3):
/// - `income`   on account_id    -> +amount
/// - `expense`  on account_id    -> -amount
/// - `transfer` on account_id    -> -amount   (money left this account)
/// - `transfer` to counter==id   -> +amount   (money arrived in this account)
///
/// Saturating arithmetic keeps an absurdly large slice from panicking.
pub fn compute_balance(initial_balance: i64, txs: &[Transaction], account_id: i64) -> i64 {
    let mut balance = initial_balance;
    for tx in txs {
        match tx.type_ {
            TxType::Income if tx.account_id == account_id => {
                balance = balance.saturating_add(tx.amount);
            }
            TxType::Expense if tx.account_id == account_id => {
                balance = balance.saturating_sub(tx.amount);
            }
            TxType::Transfer if tx.account_id == account_id => {
                balance = balance.saturating_sub(tx.amount);
            }
            TxType::Transfer if tx.counter_account_id == Some(account_id) => {
                balance = balance.saturating_add(tx.amount);
            }
            _ => {}
        }
    }
    balance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx(t: TxType, amount: i64, account_id: i64, counter: Option<i64>) -> Transaction {
        Transaction {
            id: 0,
            occurred_on: "2026-05-25".into(),
            type_: t,
            amount,
            account_id,
            counter_account_id: counter,
            category_id: if matches!(t, TxType::Transfer) { None } else { Some(1) },
            description: String::new(),
            recurring_id: None,
            created_at: "2026-05-25T00:00:00Z".into(),
            updated_at: "2026-05-25T00:00:00Z".into(),
        }
    }

    #[test]
    fn empty_returns_initial() {
        assert_eq!(compute_balance(100_000, &[], 1), 100_000);
    }

    #[test]
    fn income_increases_balance_for_owning_account() {
        let txs = [tx(TxType::Income, 50_000, 1, None)];
        assert_eq!(compute_balance(0, &txs, 1), 50_000);
        // Other accounts are unaffected.
        assert_eq!(compute_balance(0, &txs, 2), 0);
    }

    #[test]
    fn expense_decreases_balance_for_owning_account() {
        let txs = [tx(TxType::Expense, 30_000, 1, None)];
        assert_eq!(compute_balance(100_000, &txs, 1), 70_000);
    }

    #[test]
    fn transfer_moves_money_from_source_to_destination() {
        let txs = [tx(TxType::Transfer, 20_000, 1, Some(2))];
        assert_eq!(compute_balance(50_000, &txs, 1), 30_000); // source -20_000
        assert_eq!(compute_balance(50_000, &txs, 2), 70_000); // destination +20_000
    }

    #[test]
    fn transfer_total_across_two_accounts_is_zero_net() {
        let txs = [tx(TxType::Transfer, 20_000, 1, Some(2))];
        let a = compute_balance(50_000, &txs, 1);
        let b = compute_balance(50_000, &txs, 2);
        assert_eq!((a + b) - (50_000 + 50_000), 0);
    }

    #[test]
    fn unrelated_transactions_have_no_effect() {
        let txs = [
            tx(TxType::Income, 1_000, 99, None),
            tx(TxType::Expense, 500, 99, None),
            tx(TxType::Transfer, 100, 99, Some(98)),
        ];
        assert_eq!(compute_balance(1234, &txs, 1), 1234);
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    fn tx_strategy(my_account: i64, other: i64) -> impl Strategy<Value = Transaction> {
        // For transfers, the proptest invariant assumes either both legs are
        // within {my_account, other} or both legs are outside it. A transfer
        // that straddles the {1,2} boundary breaks conservation (it leaks
        // money in or out of the tracked pair) and is not the case this
        // property is testing. We therefore generate transfer source/dest
        // from a single pool per generated row, with a different pool per
        // row chosen via `prop_oneof`.
        let unrelated_a = other + 100;
        let unrelated_b = other + 200;
        let non_transfer = (
            prop_oneof![Just(TxType::Income), Just(TxType::Expense)],
            1i64..1_000_000,
            prop_oneof![Just(my_account), Just(other), Just(unrelated_a)],
        )
            .prop_map(move |(t, amt, src)| Transaction {
                id: 0,
                occurred_on: "2026-05-25".into(),
                type_: t,
                amount: amt,
                account_id: src,
                counter_account_id: None,
                category_id: Some(1),
                description: String::new(),
                recurring_id: None,
                created_at: "2026-05-25T00:00:00Z".into(),
                updated_at: "2026-05-25T00:00:00Z".into(),
            });
        let transfer_within = (
            1i64..1_000_000,
            prop_oneof![
                Just((my_account, other)),
                Just((other, my_account)),
                Just((unrelated_a, unrelated_b)),
                Just((unrelated_b, unrelated_a)),
            ],
        )
            .prop_map(|(amt, (src, dst))| Transaction {
                id: 0,
                occurred_on: "2026-05-25".into(),
                type_: TxType::Transfer,
                amount: amt,
                account_id: src,
                counter_account_id: Some(dst),
                category_id: None,
                description: String::new(),
                recurring_id: None,
                created_at: "2026-05-25T00:00:00Z".into(),
                updated_at: "2026-05-25T00:00:00Z".into(),
            });
        prop_oneof![non_transfer, transfer_within]
    }

    proptest! {
        #[test]
        fn transfers_within_two_accounts_preserve_total(
            txs in proptest::collection::vec(tx_strategy(1, 2), 0..200),
            initial_a in 0i64..1_000_000,
            initial_b in 0i64..1_000_000,
        ) {
            let bal_a = compute_balance(initial_a, &txs, 1);
            let bal_b = compute_balance(initial_b, &txs, 2);

            // Sum of income on {1,2} minus sum of expense on {1,2}.
            let net: i64 = txs.iter().fold(0i64, |acc, t| match t.type_ {
                TxType::Income  if t.account_id == 1 || t.account_id == 2 =>
                    acc.saturating_add(t.amount),
                TxType::Expense if t.account_id == 1 || t.account_id == 2 =>
                    acc.saturating_sub(t.amount),
                _ => acc,
            });

            prop_assert_eq!(bal_a + bal_b, initial_a + initial_b + net);
        }
    }
}
