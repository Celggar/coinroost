-- Recovery codes: a set of ten single-use codes per user, for a login without the authenticator app (NIST SP 800-63B-4
-- look-up secrets). Each code is stored only as an Argon2id hash, like a password; a new set replaces the old one.
create table recovery_codes (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users (id) on delete cascade,
    -- Argon2id, PHC string format.
    code_hash text not null,
    used_at timestamptz,
    created_at timestamptz not null default now()
);
create index recovery_codes_user on recovery_codes (user_id);

-- A login with a recovery code retires the authenticator: until a new one is confirmed its codes are refused and the
-- user's sessions may only set up the new one. The secret being set up waits here until a code from it arrives.
alter table users add column reenroll_required boolean not null default false;
alter table users add column pending_totp_secret bytea check (length(pending_totp_secret) >= 16);
