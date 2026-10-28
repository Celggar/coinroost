-- Coinroost schema. A household (one or several users) owns accounts, the transactions on them, the payees and
-- categories they are sorted into, and the ordered rules that sort them. Money is an integer count of minor
-- units next to its ISO 4217 code. Every row carries its household, and references between rows include it,
-- so a row can never point at another household's data.
--
-- A transaction has one leg (income or spending on one account) or two legs (a transfer between own accounts,
-- summing to zero). Each leg keeps what its bank printed: the date, the text, the import it came from and its
-- `external_id`, the key that makes re-importing the same file add nothing.

create table households (
    id uuid primary key default gen_random_uuid(),
    name text not null check (length(name) between 1 and 200),
    created_at timestamptz not null default now()
);

create table users (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    email text not null unique check (email = lower(email) and position('@' in email) > 1),
    -- Argon2id, PHC string format.
    password_hash text not null,
    -- RFC 6238 shared secret, and the last time step a code was accepted for (a code is accepted once).
    totp_secret bytea not null check (length(totp_secret) >= 16),
    totp_last_step bigint not null default 0,
    failed_logins integer not null default 0,
    locked_until timestamptz,
    created_at timestamptz not null default now()
);

create table sessions (
    -- SHA-256 of the token; the token itself is only ever in the client's cookie or header.
    token_hash bytea primary key check (length(token_hash) = 32),
    user_id uuid not null references users (id) on delete cascade,
    created_at timestamptz not null default now(),
    expires_at timestamptz not null
);
create index sessions_user on sessions (user_id);

create table accounts (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    name text not null check (length(name) between 1 and 200),
    kind text not null check (kind in ('checking', 'savings', 'credit_card', 'cash', 'loan', 'investment')),
    currency char(3) not null check (currency ~ '^[A-Z]{3}$'),
    -- Goes into every external_id of the account's legs, so it cannot change once the account has any.
    namespace text not null check (namespace ~ '^[a-z0-9][a-z0-9_-]{0,63}$'),
    -- Bank profile its exports are read with.
    profile text,
    created_at timestamptz not null default now(),
    unique (household_id, name),
    unique (household_id, namespace),
    unique (id, household_id),
    unique (id, household_id, currency)
);

create table categories (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    name text not null check (length(name) between 1 and 100),
    created_at timestamptz not null default now(),
    unique (household_id, name),
    unique (id, household_id)
);

create table payees (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    name text not null check (length(name) between 1 and 200),
    created_at timestamptz not null default now(),
    unique (household_id, name),
    unique (id, household_id)
);

-- Ordered, first match wins. Every condition that is set must hold; a rule sets a category, a payee or both.
create table rules (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    position integer not null check (position >= 1),
    name text not null check (length(name) between 1 and 200),
    enabled boolean not null default true,
    description_contains text check (length(description_contains) >= 1),
    direction text check (direction in ('in', 'out')),
    account_id uuid,
    -- Magnitude in minor units, both ends included.
    amount_min bigint check (amount_min >= 0),
    amount_max bigint check (amount_max >= 0),
    category_id uuid,
    payee_id uuid,
    created_at timestamptz not null default now(),
    unique (household_id, position) deferrable initially immediate,
    foreign key (account_id, household_id) references accounts (id, household_id),
    foreign key (category_id, household_id) references categories (id, household_id),
    foreign key (payee_id, household_id) references payees (id, household_id),
    check (amount_min is null or amount_max is null or amount_min <= amount_max),
    check (category_id is not null or payee_id is not null),
    check (description_contains is not null or direction is not null or account_id is not null
           or amount_min is not null or amount_max is not null)
);

create table import_batches (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    account_id uuid not null,
    file_name text not null check (length(file_name) between 1 and 255),
    file_sha256 bytea not null check (length(file_sha256) = 32),
    profile text not null,
    rows_read integer not null check (rows_read >= 0),
    added integer not null check (added >= 0),
    duplicates integer not null check (duplicates >= 0),
    errors integer not null check (errors >= 0),
    created_by uuid not null references users (id),
    created_at timestamptz not null default now(),
    foreign key (account_id, household_id) references accounts (id, household_id),
    unique (id, household_id)
);

create table transactions (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    date date not null,
    description text not null check (length(description) <= 1000),
    payee_id uuid,
    category_id uuid,
    created_at timestamptz not null default now(),
    foreign key (payee_id, household_id) references payees (id, household_id),
    foreign key (category_id, household_id) references categories (id, household_id),
    unique (id, household_id)
);
create index transactions_household_date on transactions (household_id, date desc, id);

create table legs (
    id uuid primary key default gen_random_uuid(),
    transaction_id uuid not null,
    household_id uuid not null,
    account_id uuid not null,
    amount_minor bigint not null,
    currency char(3) not null,
    -- What the bank printed for this leg.
    posted_on date not null,
    statement_text text not null default '' check (length(statement_text) <= 1000),
    external_id text check (length(external_id) between 1 and 128),
    import_batch_id uuid,
    foreign key (transaction_id, household_id) references transactions (id, household_id) on delete cascade,
    foreign key (account_id, household_id, currency) references accounts (id, household_id, currency),
    foreign key (import_batch_id, household_id) references import_batches (id, household_id),
    unique (transaction_id, account_id),
    unique (account_id, external_id)
);
create index legs_transaction on legs (transaction_id);
create index legs_account_posted on legs (account_id, posted_on desc);

create function accounts_namespace_is_fixed() returns trigger language plpgsql as $$
begin
    if new.namespace <> old.namespace and exists (select 1 from legs where account_id = old.id) then
        raise exception 'the namespace of an account with transactions cannot change'
            using errcode = 'check_violation';
    end if;
    return new;
end
$$;
create trigger accounts_namespace_fixed before update of namespace on accounts
    for each row execute function accounts_namespace_is_fixed();
