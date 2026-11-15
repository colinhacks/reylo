create table t (id integer, n integer); create view v as select id, n, n + 1 as m from t; comment on column v.id is 'the id';
