create macro ok_code (c) as ((c != ''));
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, code varchar, primary key (id), check (ok_code(code)));
insert into t (id, code) select id, code from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro valid;
