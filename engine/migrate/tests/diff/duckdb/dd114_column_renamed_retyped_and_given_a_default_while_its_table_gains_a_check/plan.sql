create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, quantity bigint default 1, note varchar, primary key (id), check ((quantity >= 0)));
insert into t (id, quantity, note) select id, qty, note from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.qty is renamed quantity, keeping its rows. If they are two columns, drop the one and add the other instead.
