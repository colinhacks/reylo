create macro zz(x, y := 0) as x + 2; create table t (a integer); create view v as select a.zz() as y from t;
