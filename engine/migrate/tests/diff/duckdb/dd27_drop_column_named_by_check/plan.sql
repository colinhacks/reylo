-- loses data: t.lo
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, hi integer, primary key (id));
insert into t (id, hi) select id, hi from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
