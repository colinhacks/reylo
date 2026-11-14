drop type mood;
create type mood as enum('a', 'b', 'c');
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, m mood, primary key (id));
insert into t (id, m) select id, m from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index t_m on t(m);
