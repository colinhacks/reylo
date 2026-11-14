create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
create type mood as enum('a', 'b', 'c');
create table t (id integer, m varchar, d varchar default cast('a' as mood), check ((cast(m as mood) is not null)));
insert into t (id, m, d) select id, m, d from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
