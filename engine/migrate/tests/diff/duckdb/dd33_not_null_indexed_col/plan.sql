create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, a varchar not null, b varchar, primary key (id));
insert into t (id, a, b) select id, a, b from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index t_ab on t(a, b);
