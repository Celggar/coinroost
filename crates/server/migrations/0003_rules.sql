-- What a category holds: spending, income, or money moved between the household's own accounts.
alter table categories
    add column kind text not null default 'expense' check (kind in ('expense', 'income', 'transfer'));

-- A rule may read the payee from the text after the matched payment channel instead of naming one, so the check
-- that a rule sets something (unnamed in 0001) is replaced.
do $$
declare
    old_check text;
begin
    select conname into old_check from pg_constraint
    where conrelid = 'rules'::regclass and contype = 'c'
      and pg_get_constraintdef(oid) = 'CHECK (((category_id IS NOT NULL) OR (payee_id IS NOT NULL)))';
    if old_check is null then
        raise exception 'the check that a rule sets a category or a payee was not found';
    end if;
    execute format('alter table rules drop constraint %I', old_check);
end $$;

alter table rules
    add column payee_from_text boolean not null default false,
    add constraint rules_sets_something check (category_id is not null or payee_id is not null or payee_from_text),
    add constraint rules_payee_from_text check (
        not payee_from_text or (payee_id is null and description_contains is not null)),
    add constraint rules_id_household unique (id, household_id);

-- Who set a transaction's category and payee: a rule (which one, while it exists) or a person. Applying rules
-- again never touches what a person set.
alter table transactions
    add column category_set_by text check (category_set_by in ('rule', 'manual')),
    add column payee_set_by text check (payee_set_by in ('rule', 'manual')),
    add column rule_id uuid;

update transactions set category_set_by = 'manual' where category_id is not null;
update transactions set payee_set_by = 'manual' where payee_id is not null;

alter table transactions
    add constraint transactions_category_set_by check ((category_id is null) = (category_set_by is null)),
    add constraint transactions_payee_set_by check ((payee_id is null) = (payee_set_by is null)),
    add constraint transactions_rule foreign key (rule_id, household_id)
        references rules (id, household_id) on delete set null (rule_id);

create index transactions_uncategorized on transactions (household_id, date desc) where category_id is null;
