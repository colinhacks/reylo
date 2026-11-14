-- JSON a query builds beside an aggregate: its values outside the aggregate's arguments are probed
-- under the query's GROUP BY, a scalar subquery's too. Over no rows a query that aggregates with no
-- GROUP BY returns one row of NULL beside its aggregates, and WITH ROLLUP nulls what it groups by in
-- its super-aggregate rows, in the row a subquery reads too.
-- Without ONLY_FULL_GROUP_BY, a bare column may stand beside an aggregate.
create table users (id int primary key, name varchar(20) not null, email varchar(40), flag boolean not null default true, score decimal(10,2), ratio double, born date, seen datetime, at time, doc json, raw varbinary(8), kind enum('a','b'), bits bit(8), y year, big bigint, tags set('x','y'), n int not null default 1);
create table posts (id int primary key, user_id int not null, title varchar(20) not null, tag varchar(20), score int, n int not null default 1, key (user_id));
create table nothing (id int primary key, v int not null, check (false));
