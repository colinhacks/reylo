create macro z_new (x) as ((x > 0));
create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro a_outer;
create macro a_outer (x) as (z_new(x));
create table t (id integer, n integer, primary key (id), check (a_outer(n)));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro z_old;
