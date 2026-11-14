create table t (id integer primary key);
create table counters (id serial primary key, n integer);
create view v as select id from t;
create function f() returns integer language sql as 'select 1';
