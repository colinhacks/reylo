-- An aggregate inside JSON the statement builds sits beside the statement's own columns, which
-- only its GROUP BY makes valid together: the probe of them carries it, and an aggregate's
-- arguments go in a probe without one. A GROUP BY by position or by a name of the select list
-- cannot be carried.
create table users (id bigint primary key, name text not null);
create table posts (id bigint primary key, user_id bigint not null references users, title text not null, score int);
