create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, price integer, qty integer, total integer generated always as (((price * qty) + 1)) virtual, primary key (id));
insert into t (id, price, qty) select id, price, qty from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
