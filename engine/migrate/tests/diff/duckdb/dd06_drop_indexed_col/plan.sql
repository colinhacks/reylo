-- loses data: t.b
drop index t_b;
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, a varchar, primary key (id));
insert into t (id, a) select id, a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
