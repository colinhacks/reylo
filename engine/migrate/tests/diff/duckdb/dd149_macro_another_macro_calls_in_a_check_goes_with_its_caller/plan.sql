create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, qty integer, primary key (id), check (((qty > 0) and (qty < 100))));
insert into t (id, qty) select id, qty from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro is_pos;
drop macro ok_qty;
