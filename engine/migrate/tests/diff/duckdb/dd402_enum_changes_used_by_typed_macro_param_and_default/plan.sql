create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
drop macro ok;
create macro ok (x enum('a', 'b', 'c')) as ((x is not null));
create type mood as enum('a', 'b', 'c');
create table t (a varchar default 'a', b mood default 'b', check (ok(cast(a as mood))));
insert into t (a, b) select a, b from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
