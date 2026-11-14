create table t (id integer primary key, n integer); create macro top(k) as table select * from t order by n desc limit k; create view v as from top(2);
