create table t (id integer primary key, a integer, g integer generated always as (a * 2) virtual); create index ta on t (a);
