create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, "Order" integer, "select" varchar, r struct(A integer), n integer, primary key (id), check ((n > 0)), check (("Order" > 0)), check (("select" != '')), check ((r['A'] > 0)));
insert into t (id, "Order", "select", r) select id, "Order", "select", r from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
