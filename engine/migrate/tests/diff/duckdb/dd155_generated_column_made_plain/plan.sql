create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, a integer, b integer, total integer not null, twice integer, c integer, primary key (id));
insert into t (id, a, b, total, twice, c) select id, a, b, total, twice, c from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
