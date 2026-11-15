create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
drop macro is_a;
create macro is_a (m enum('a', 'b', 'c')) as ((m = 'a'));
create type mood as enum('a', 'b', 'c');
create table t (id integer, m mood, primary key (id), check ((is_a(m) or (m != 'a'))));
insert into t (id, m) select id, m from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
