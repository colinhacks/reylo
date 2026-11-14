create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro bump;
create macro bump (a, b) as ((a + b));
create table t (id integer, n integer, m integer generated always as (bump(n, 2)) virtual, primary key (id));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
