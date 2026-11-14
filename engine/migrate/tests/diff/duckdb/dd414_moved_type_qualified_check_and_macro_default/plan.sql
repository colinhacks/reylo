create temporary table reylo_copy_t as select * from t;
drop table t;
drop type s.mood;
drop macro m;
create macro m (x) as ((x + 2));
create type s.mood as enum('a', 'b', 'c');
create table t (a integer default m(1), b varchar, check ((cast(b as s.mood) is not null)));
insert into t (a, b) select a, b from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
