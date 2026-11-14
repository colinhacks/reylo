create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
create type mood as enum('a', 'b', 'c');
create table t (id integer, s varchar, primary key (id), check ((cast(s as MOOD) != 'b')));
insert into t (id, s) select id, s from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
