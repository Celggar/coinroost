-- A fourth kind: what an account held on the day its history starts. It counts in the account's balance and in no
-- income, spending or pairing, as double-entry books keep opening balances in equity.
alter table categories
    drop constraint categories_kind_check,
    add constraint categories_kind_check check (kind in ('expense', 'income', 'transfer', 'opening_balance'));
