create macro zz(x) as x + 1; create table t (a integer); create view v as select a.zz() as y from t;
