create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, price integer, qty integer, total integer generated always as ((price * qty)) virtual, primary key (id), check ((price >= 0)));
insert into t (id, price, qty) select id, price, qty from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
