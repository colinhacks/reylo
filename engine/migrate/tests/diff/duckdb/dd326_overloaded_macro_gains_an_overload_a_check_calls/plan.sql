create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m;
create macro m (a) as ((a + 1)), (a, b) as ((a + b));
create table t (id integer, n integer, primary key (id), check (((m(n) > 0) and (m(n, 1) > 0))));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
