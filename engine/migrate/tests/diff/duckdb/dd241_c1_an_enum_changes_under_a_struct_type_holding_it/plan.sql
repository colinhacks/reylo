create temporary table reylo_copy_t as select * from t;
drop table t;
drop type rec;
drop type mood;
create type mood as enum('a', 'b', 'c');
create type rec as struct(m enum('a', 'b', 'c'), n integer);
create table t (id integer, r rec, s varchar, primary key (id), check (((cast(s as rec) is not null) or (s is null))));
insert into t (id, r, s) select id, r, s from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
