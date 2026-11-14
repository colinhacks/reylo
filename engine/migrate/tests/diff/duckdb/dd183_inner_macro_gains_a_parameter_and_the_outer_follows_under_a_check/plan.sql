create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro a_outer;
drop macro z_inner;
create macro z_inner (x, k) as ((x > k));
create macro a_outer (x) as (z_inner(x, 0));
create table t (id integer, n integer, primary key (id), check (a_outer(n)));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
