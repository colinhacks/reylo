create table t (id integer primary key, a integer, b integer generated always as (a * 2) stored);
