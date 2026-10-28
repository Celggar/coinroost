-- Long-lived tokens for programs that act for a user (an MCP client, a script). Each is 256 random bits shown
-- once; only its SHA-256 is stored, like a session's. A `read` token may only read.
create table api_tokens (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    name text not null check (length(trim(name)) between 1 and 100),
    token_hash bytea not null unique check (length(token_hash) = 32),
    scope text not null check (scope in ('read', 'write')),
    created_at timestamptz not null default now(),
    last_used_at timestamptz,
    revoked_at timestamptz
);

-- A name names one live token per user; a revoked token keeps its row (and its name) for the record.
create unique index api_tokens_live_name on api_tokens (user_id, name) where revoked_at is null;
