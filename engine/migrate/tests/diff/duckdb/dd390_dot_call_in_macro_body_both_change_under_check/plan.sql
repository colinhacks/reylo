create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro a_outer;
drop macro z_inner;
create macro z_inner (x) as ((x + 2));
create macro a_outer (x) as ((x.z_inner() > -1));
create table t (a integer, check (a_outer(a)));
insert into t (a) select a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
