create table t (id integer primary key);
alter table t owner to owner_role;
create table counters (id serial primary key, n integer);
alter table counters owner to owner_role;
create view v as select id from t;
alter view v owner to owner_role;
create function f() returns integer language sql as 'select 1';
alter function f() owner to owner_role;
