create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, a integer, primary key (id), check ((a > 0)));
insert into t (id, a) select id, a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
