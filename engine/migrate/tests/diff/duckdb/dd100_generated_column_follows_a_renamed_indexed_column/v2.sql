create table t (id integer primary key, b integer, g integer generated always as (b * 2) virtual); create index ta on t (b);
