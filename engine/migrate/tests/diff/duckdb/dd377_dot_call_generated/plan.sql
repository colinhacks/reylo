create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m;
create macro m (x) as ((x + 2));
create table t (a integer, b integer generated always as (a.m()) virtual);
insert into t (a) select a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
