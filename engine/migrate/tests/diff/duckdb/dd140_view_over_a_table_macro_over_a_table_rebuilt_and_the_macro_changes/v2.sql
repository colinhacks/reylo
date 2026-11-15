create table t (id integer primary key, n integer check (n > 0)); create macro top(k) as table select id, n from t order by n desc limit k; create view v as from top(2);
