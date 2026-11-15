drop index t_i;
create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
create type mood as enum('a', 'b', 'c');
create table t (id integer, m varchar, n integer, primary key (id), check ((cast(m as mood) != 'b')));
insert into t (id, m, n) select id, m, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index t_i on t(m);
