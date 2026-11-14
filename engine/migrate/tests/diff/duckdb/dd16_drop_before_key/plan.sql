-- loses data: t.gone
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, k integer, primary key (id), unique (k));
insert into t (id, k) select id, k from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
