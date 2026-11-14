drop index t_n;
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, n bigint, primary key (id));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
