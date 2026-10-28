-- Employers that pay by the fortnight, for the report of what each still owes. Loaded from a file by name.
create table employers (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    name text not null check (length(name) between 1 and 200),
    -- Text its deposits carry in their description, compared without case.
    match_text text not null check (length(match_text) between 1 and 200),
    -- The account its deposits arrive in.
    account_id uuid not null,
    -- Below this a deposit is not a fortnight's pay (a 13th-month bonus is smaller).
    min_amount_minor bigint not null check (min_amount_minor > 0),
    -- What a fortnight is worth today.
    amount_minor bigint not null check (amount_minor > 0),
    -- The first month counted, from its first fortnight.
    start_month date not null check (extract(day from start_month) = 1),
    created_at timestamptz not null default now(),
    unique (household_id, name),
    foreign key (account_id, household_id) references accounts (id, household_id)
);
