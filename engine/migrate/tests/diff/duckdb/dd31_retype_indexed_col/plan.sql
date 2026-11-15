create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, n bigint, a varchar, primary key (id));
insert into t (id, n, a) select id, n, a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index t_n on t(n);
