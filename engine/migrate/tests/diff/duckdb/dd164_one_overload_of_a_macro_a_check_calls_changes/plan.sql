create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro ok;
create macro ok (a) as ((a > 0)), (a, b) as ((a >= b));
create table t (id integer, v integer, primary key (id), check (ok(v)));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
