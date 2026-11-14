create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
create type mood as enum('a', 'b', 'c');
create table t (id integer, s varchar, m varchar generated always as (cast(as_mood(s) as VARCHAR)) virtual, primary key (id));
insert into t (id, s) select id, s from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
