create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, ticket bigint default nextval('s'), n integer, primary key (id), check ((n > 0)));
insert into t (id, ticket, n) select id, k, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.k is renamed ticket, keeping its rows. If they are two columns, drop the one and add the other instead.
