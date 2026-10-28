-- How each account's exports are recognized, so files from several accounts can be dropped at once (the
-- normalizer's detect module): texts the header must contain, card network prefixes for cards that share a
-- format, the account type label the bank prints, and the first 16 hex digits of the SHA-256 of the account
-- number.
alter table accounts
    add column detect_markers text[] not null default '{}',
    add column detect_card_prefixes text[] not null default '{}'
        check (array_to_string(detect_card_prefixes, ',') ~ '^([0-9]{1,6}(,[0-9]{1,6})*)?$'),
    add column detect_label text check (length(detect_label) between 1 and 200),
    add column detect_number_hash text check (detect_number_hash ~ '^[0-9a-f]{16}$');

-- The dates a file covers: the first and last date among every row read, duplicates included, so coverage shows
-- what was imported even when all its rows were already there. Empty when no row could be read.
alter table import_batches
    add column first_date date,
    add column last_date date,
    add constraint import_batches_dates
        check ((first_date is null) = (last_date is null) and first_date <= last_date);

-- Imports made before this migration: the dates of the legs they added.
update import_batches b
set first_date = l.first_date, last_date = l.last_date
from (
    select import_batch_id, min(posted_on) as first_date, max(posted_on) as last_date
    from legs
    where import_batch_id is not null
    group by import_batch_id
) l
where l.import_batch_id = b.id;
