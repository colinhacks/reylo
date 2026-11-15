create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro ok;
create macro ok (x) as ((x >= 0));
create table t (id integer, v integer, check ((v > 0)));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
