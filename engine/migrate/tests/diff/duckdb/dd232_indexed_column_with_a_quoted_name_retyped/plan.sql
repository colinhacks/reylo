create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, Score bigint, primary key (id));
insert into t (id, Score) select id, Score from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index t_score on t(Score);
