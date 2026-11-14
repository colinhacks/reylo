drop index ta;
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, b integer, g integer generated always as ((b * 2)) virtual, primary key (id));
insert into t (id, b) select id, a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.a is renamed b, keeping its rows. If they are two columns, drop the one and add the other instead.
create index ta on t(b);
