create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
create type mood as enum('a', 'b', 'c');
create table t (id integer, m mood, s varchar, primary key (id), check ((cast(s as mood) != 'b')));
insert into t (id, m, s) select id, m, s from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index t_m on t(m);
