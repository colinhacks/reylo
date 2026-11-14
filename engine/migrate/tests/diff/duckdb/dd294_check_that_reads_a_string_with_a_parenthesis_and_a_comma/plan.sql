create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, s varchar, n integer, primary key (id), check ((not (s in (')', ',', '(')))));
insert into t (id, s, n) select id, s, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
