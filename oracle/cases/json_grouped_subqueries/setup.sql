-- JSON built in a scalar subquery that groups or aggregates: its values outside an aggregate's
-- arguments are probed under the subquery's own GROUP BY, only where every call in its select item
-- is its own. Aggregates of the query around it, rows a grouping set of an outer query nulls, and
-- subqueries in every place a statement holds one.
create table users (id bigint primary key, name text not null, title text not null, active boolean not null default true);
create table posts (id bigint primary key, user_id bigint not null references users, title text not null, score int, tag text, n int not null default 1);
create table nothing (id bigint primary key, v int not null, check (false));
