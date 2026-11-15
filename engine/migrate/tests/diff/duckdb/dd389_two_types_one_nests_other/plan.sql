drop type pair;
drop type mood;
create type mood as enum('a', 'b', 'c');
create type pair as struct(m enum('a', 'b', 'c'), n integer);
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (p pair, check ((((p).n > 0) or (p is null))));
insert into t (p) select p from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
