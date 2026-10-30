-- The first account on a fresh install: while there are no users, anyone who reaches the app can ask for a setup
-- link that makes a new household, the way a fresh Firefly III lets its first visitor register. Once any user
-- exists, those links stop working (the server checks it under a lock when one is accepted).
alter table invites add column first_account boolean not null default false;
alter table invites add constraint invites_first_account_new_household
    check (not first_account or household_id is null);
