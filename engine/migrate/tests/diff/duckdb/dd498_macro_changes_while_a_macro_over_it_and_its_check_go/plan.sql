create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m;
create macro m (x) as ((x > -1));
create table t (id integer, n integer, check (m(n)));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro outer_ok;
