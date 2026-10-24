-- An aggregate used as a window function is NULL only where no non-null value reaches its frame.
-- A frame that always holds the current row (no frame clause, or one that starts at or before
-- the current row and ends at or after it, excluding neither it nor its group) and no FILTER
-- leave it that row's value, so `min`, `max`, `group_concat` and `string_agg` follow their
-- argument; any other frame can be empty. A `sum` or `avg` stays nullable as under GROUP BY: a sum
-- of 1e999 and -1e999 is NaN, which SQLite returns as NULL. The ordering columns hold NULL, the
-- infinities, text and blobs, which a RANGE offset meets.
create table users (id integer primary key, name text not null);
create table posts (id integer primary key, user_id integer not null references users (id), title text not null, published_at text, score real, rank, note text);
insert into users (name) values ('Ada'), ('Grace');
insert into posts (user_id, title, published_at, score, rank, note) values (1, 'Hello', '2026-01-01T00:00:00.000Z', 1.5, 2, 'n'), (1, 'Draft', null, null, null, null), (2, 'Notes', '2026-02-03T00:00:00.000Z', 1e999, 'x', null);
