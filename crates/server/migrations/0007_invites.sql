-- Single-use setup links: how a person gets their user from the web, with no open sign-up.
create table invites (
    id uuid primary key default gen_random_uuid(),
    -- SHA-256 of the token; the token itself is only ever in the link (its fragment) and in request bodies.
    token_hash bytea not null unique check (length(token_hash) = 32),
    -- The household the new user joins; null makes a new household on acceptance.
    household_id uuid references households (id),
    -- The authenticator secret the server generated for this link, kept until the user is created with it.
    totp_secret bytea check (length(totp_secret) >= 16),
    failed_attempts integer not null default 0,
    expires_at timestamptz not null,
    used_at timestamptz,
    used_by uuid references users (id) on delete set null,
    created_at timestamptz not null default now(),
    check (used_by is null or used_at is not null)
);
