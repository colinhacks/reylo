create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, email varchar, primary key (id), unique (email));
insert into t (id, email) select id, email from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
