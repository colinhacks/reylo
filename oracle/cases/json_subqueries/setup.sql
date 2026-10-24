-- A JSON constructor's shape where a name or a subquery decides it. A bare name is a column
-- before it is a FROM item, of the innermost query that has one of that name, so `to_jsonb(u)` is
-- a whole row only where no FROM in scope has a column `u`: `users` has one, and a column `p`.
-- And a scalar subquery that may find no row is NULL whatever its value is: a column, a whole
-- row, or an aggregate of the query around it. One that aggregates its own rows is one row, an
-- aggregate of a whole row by its bare name `q` included, which no FROM in scope has a column of;
-- an `array_agg(q)` there is of stored rows of `posts`, which keep its NOT NULL.
-- Inside a subquery over `posts p`, a bare `p` is the column of `users`, which a value read over
-- the subquery's FROM alone would take for the row of `posts`.
create table users (id bigint primary key, name text not null, u jsonb, p text);
create table posts (id bigint primary key, user_id bigint not null, title text not null);
