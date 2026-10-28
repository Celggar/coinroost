-- Where a transaction came from when another system held it first (a migration), so running the migration again
-- adds nothing. Statement imports do not need it: their legs carry an external_id.
create table transaction_origins (
    transaction_id uuid primary key,
    household_id uuid not null,
    source text not null check (source in ('firefly')),
    source_id text not null check (length(source_id) between 1 and 100),
    foreign key (transaction_id, household_id) references transactions (id, household_id) on delete cascade,
    unique (household_id, source, source_id)
);
