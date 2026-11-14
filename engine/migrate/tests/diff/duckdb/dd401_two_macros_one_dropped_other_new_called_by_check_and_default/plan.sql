create macro e (x) as ((x + 2));
create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m;
create macro m (x) as ((x > -1));
create table t (a integer default e(1), b integer, check (m(b)));
insert into t (a, b) select a, b from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro d;
