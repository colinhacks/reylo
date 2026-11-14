create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
create type mood as enum('a', 'b', 'c');
create table t (id integer, m varchar default cast('a' as mood), primary key (id), check ((cast(m as main.mood) != 'b')));
insert into t (id, m) select id, m from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
