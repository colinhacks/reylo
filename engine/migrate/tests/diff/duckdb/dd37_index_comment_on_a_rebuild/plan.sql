create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, a varchar, primary key (id), check ((a != '')));
insert into t (id, a) select id, a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index t_a on t(a);
comment on index t_a is 'lookup';
