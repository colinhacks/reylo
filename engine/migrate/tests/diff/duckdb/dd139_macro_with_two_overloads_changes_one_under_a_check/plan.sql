create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro f;
create macro f (a) as ((a >= 0)), (a, b) as ((a > b));
create table t (id integer, n integer, m integer, primary key (id), check (f(n)), check (f(m, 1)));
insert into t (id, n, m) select id, n, m from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
