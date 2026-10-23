-- What `.try()`'s error type is narrowed to on SQLite: the unique keys, NOT NULL columns,
-- STRICT columns and expression indexes a statement writes through, and nothing where it may fire
-- a trigger.
create table users (id integer primary key, email text unique not null, age integer check (age >= 18)) strict;
create unique index users_lower_email on users (lower(email));
create table posts (id integer primary key, user_id integer not null references users (id) on delete cascade, slug text, lang text, unique (slug, lang));
create table pairs (x integer, y integer, primary key (x, y)) without rowid;
create table audited (id integer primary key, n integer);
create trigger audited_audit after insert on audited begin update audited set n = 1 where id = new.id; end;
insert into users (id, email, age) values (1, 'ada@example.com', 30);
insert into posts (id, user_id, slug, lang) values (1, 1, 'hello', 'en');
