create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro dbl;
create macro dbl (x) as ((x * 3));
create table t (id integer, n integer, d integer generated always as (dbl(n)) virtual, remark varchar, primary key (id));
insert into t (id, n, remark) select id, n, note from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.note is renamed remark, keeping its rows. If they are two columns, drop the one and add the other instead.
