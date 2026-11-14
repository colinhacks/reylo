create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, Name varchar not null, primary key (id));
insert into t (id, Name) select id, Name from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index t_name on t("Name");
