create temporary table reylo_copy_t as select * from t;
drop table t;
drop type s.mood;
create type s.mood as enum('a', 'b', 'c');
create table t (a varchar default cast(cast('a' as s.mood) as VARCHAR), b varchar, check ((cast(b as s.mood) is not null)));
insert into t (a, b) select a, b from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
