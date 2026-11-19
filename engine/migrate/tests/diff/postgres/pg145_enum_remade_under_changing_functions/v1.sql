create type mood as enum ('a', 'b');
create function gone(m mood) returns integer language sql immutable as 'select 1';
create function pick() returns mood language sql immutable as $$select 'a'::mood$$;
create function score(m mood) returns integer language sql immutable as 'select 2';
revoke all on function score(mood) from public;
comment on function score(mood) is 'scores a mood';
create table t (id integer primary key, m mood default pick(), s integer default score('a') check (s > 0));
create view v as select id, pick() as p from t;
