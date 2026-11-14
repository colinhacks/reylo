alter table t alter column s drop default;
drop macro dflt;
create macro dflt () as ('y');
alter table t alter column s set default dflt();
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, label varchar default dflt(), n integer, primary key (id), check ((n > 0)));
insert into t (id, label, n) select id, s, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.s is renamed label, keeping its rows. If they are two columns, drop the one and add the other instead.
