-- An aggregate used as a window function is NULL only where no non-null value reaches its frame.
-- A frame that always holds the current row (no frame clause, or one that starts at or before
-- the current row and ends at or after it, excluding neither it nor its group) and no FILTER
-- leave it that row's value, so it follows its argument; any other frame can be empty. The
-- ordering columns hold NULL, NaN and the infinities, which a RANGE offset meets.
create table users (id serial primary key, name text not null);
create table posts (id serial primary key, user_id integer not null references users (id), title text not null, published_at timestamptz, score float8, weight numeric, note text);
insert into users (name) values ('Ada'), ('Grace');
insert into posts (user_id, title, published_at, score, weight, note) values (1, 'Hello', '2026-01-01', 1.5, 2, 'n'), (1, 'Draft', null, null, null, null), (2, 'Notes', '2026-02-03', 'NaN', 'NaN', null);
