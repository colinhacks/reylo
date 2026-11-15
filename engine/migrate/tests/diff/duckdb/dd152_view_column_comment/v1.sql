create table t (x integer, y integer); create view v as select x, y from t; comment on column v.y is 'old';
