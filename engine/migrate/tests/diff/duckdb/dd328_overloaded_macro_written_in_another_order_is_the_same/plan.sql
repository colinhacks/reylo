create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m;
create macro m (a, b) as ((a + b)), (a) as ((a + 1));
create table t (id integer, n integer, primary key (id), check ((m(n) > 0)));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
