create type feeling as enum('a', 'b');
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, m varchar, primary key (id), check ((cast(m as feeling) is not null)));
insert into t (id, m) select id, m from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop type mood;
