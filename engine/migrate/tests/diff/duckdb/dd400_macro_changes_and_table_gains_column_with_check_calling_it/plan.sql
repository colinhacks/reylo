create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m;
create macro m (x) as ((x > -1));
create table t (a integer default 1, b integer, c integer default 3, check (m(b)), check (m(c)));
insert into t (a, b) select a, b from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
