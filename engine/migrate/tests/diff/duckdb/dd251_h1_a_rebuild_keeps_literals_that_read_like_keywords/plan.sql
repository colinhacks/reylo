create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, s varchar default 'NULL AND SELECT', k varchar default 'INTEGER', j json default '{"A": "B"}', n integer, primary key (id), check ((n > 0)), check ((s != 'DROP TABLE t')), check ((k in ('VARCHAR', 'INTEGER', 'Integer'))), check ((json_extract_string(j, '$.A') != 'X')));
insert into t (id, s, k, j) select id, s, k, j from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
