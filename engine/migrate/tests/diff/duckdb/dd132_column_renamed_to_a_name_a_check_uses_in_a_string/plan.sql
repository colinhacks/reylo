create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, state varchar, primary key (id), check ((state != 'state')));
insert into t (id, state) select id, status from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.status is renamed state, keeping its rows. If they are two columns, drop the one and add the other instead.
