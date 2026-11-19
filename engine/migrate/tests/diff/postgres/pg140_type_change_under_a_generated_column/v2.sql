create table t (id integer primary key, a bigint, b bigint generated always as (a * 2) stored);
