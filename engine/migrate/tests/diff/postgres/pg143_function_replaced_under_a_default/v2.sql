create function f() returns bigint language sql immutable as 'select 1';
create table t (id integer primary key, n integer default f(), m integer check (m > f()));
create view v as select id, f() as k from t;
