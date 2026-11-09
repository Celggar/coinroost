-- What a person tells about the app while using it: a problem or an idea, a message, and screenshots, each with its own
-- note. It stays in the household's own database; nothing is sent anywhere.
create table feedback (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    author_id uuid references users (id) on delete set null,
    -- Made by the client for the draft, so a send retried after a lost answer makes no second report.
    report_id uuid not null,
    kind text not null check (kind in ('problem', 'idea')),
    message text not null check (length(message) <= 4000),
    -- Where it was sent from: the screen's path, its size, colour scheme, language and browser.
    context jsonb not null,
    resolved_at timestamptz,
    created_at timestamptz not null default now(),
    unique (household_id, report_id)
);
create index feedback_household on feedback (household_id, created_at desc);

create table feedback_shots (
    feedback_id uuid not null references feedback (id) on delete cascade,
    position smallint not null check (position between 0 and 9),
    -- The type read from the image's own bytes, never the one a client claims.
    media_type text not null check (media_type in ('image/jpeg', 'image/png')),
    image bytea not null check (length(image) between 1 and 600000),
    note text not null check (length(note) <= 500),
    primary key (feedback_id, position)
);
