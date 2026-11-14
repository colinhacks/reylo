create table t (id integer, n integer); create view v as select id, n from t; comment on column v.id is 'the id';
