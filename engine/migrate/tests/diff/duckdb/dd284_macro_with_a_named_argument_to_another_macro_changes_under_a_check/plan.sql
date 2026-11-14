create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro inc;
create macro inc (x, k := 1) as (((x + k) + 0));
create table t (id integer, n integer, primary key (id), check (pos(n)));
insert into t (id, n) select id, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
