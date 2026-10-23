-- Postgres's aggregates beyond count, sum, avg, min, max and the JSON arrays: without a GROUP BY
-- each returns exactly one row, NULL over no rows, so a scalar subquery over one, under a
-- coalesce with a non-null default, is never NULL. `drafts` never holds a row.
create table users (id bigint primary key, name text not null);
create table posts (id bigint primary key, user_id bigint not null references users (id), title text not null);
create table drafts (id bigint primary key, title text not null, check (false));
