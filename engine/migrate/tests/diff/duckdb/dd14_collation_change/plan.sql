create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, name varchar collate nocase, primary key (id));
insert into t (id, name) select id, name from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
