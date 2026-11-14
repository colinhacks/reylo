create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro is_pos;
create macro is_pos (x) as ((x >= 1));
create table t (id integer, qty integer, primary key (id), check (ok_qty(qty)));
insert into t (id, qty) select id, qty from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
