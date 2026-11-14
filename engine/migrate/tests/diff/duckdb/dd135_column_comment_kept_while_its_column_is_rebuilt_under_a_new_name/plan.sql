comment on column t.qty is null;
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, amount integer, primary key (id), check ((amount > 0)));
insert into t (id, amount) select id, qty from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.qty is renamed amount, keeping its rows. If they are two columns, drop the one and add the other instead.
comment on column t.amount is 'how many';
