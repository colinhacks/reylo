create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
create type mood as enum('a', 'b', 'c');
create table t (a varchar, check (ok(a)));
insert into t (a) select a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
