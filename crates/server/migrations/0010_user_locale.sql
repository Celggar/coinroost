-- The language a user picked in the app, as a BCP 47 primary tag the web app has texts for. Null until they pick
-- one, so the browser's language decides; once saved it follows them to every device after login.
alter table users add column locale text check (locale in ('es', 'en'));
