-- Nested rows as Postgres's own arrays of rows: every spelling, what each field's type and nullability are, and an array of rows inside an array of rows.
create type mood as enum ('ok', 'sad, "very"');
create domain pos as int check (value > 0);
create type addr as (street text, zip int);
create table users (id bigint primary key, name text not null);
create table posts (id bigint primary key, author_id bigint not null references users, title text not null);
create table comments (id bigint primary key, post_id bigint not null references posts, body text, big bigint not null, amount numeric not null, at timestamptz not null, d date, raw bytea, m mood, p pos, j jsonb, tags text[], a addr, f float8, iv interval, ok bool);
create table replies (id bigint primary key, comment_id bigint not null references comments, body text not null);
create view comment_view as select id, post_id, body from comments;
create function comments_for(p bigint) returns table (id bigint, body text) language sql stable as $$ select id, body from comments where post_id = p $$;
insert into users values (1, 'ada');
insert into posts values (1, 1, 'has'), (2, 1, 'none');
insert into comments values (1, 1, 'he said "hi", (ok) \ back', 9007199254740993, 1234567890.1234567891, '2026-11-05 04:00:00.123456-08', '0044-03-15 BC', '\x0102ff', 'sad, "very"', 5, '{"n": 9007199254740993}', '{a,"b c",NULL,"NULL",""}', row('1 "Main", St', 12345), 'NaN', '1 day 02:03:04', true), (2, 1, '', 0, 0, 'infinity', null, '\x', null, null, null, '{}', null, null, null, null), (3, 1, null, 1, 1, '2026-01-01 00:00:00+00', null, null, null, null, null, null, row(null, null), 'Infinity', null, false);
insert into replies values (1, 1, 'r "1"'), (2, 1, 'r2');
