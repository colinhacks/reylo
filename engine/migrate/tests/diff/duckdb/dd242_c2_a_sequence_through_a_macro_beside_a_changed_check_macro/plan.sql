-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
create temporary table reylo_copy_t as select * from t;
drop table t;
drop sequence s;
drop macro ok;
create macro ok (v) as ((v > 1));
create sequence s start with 1 increment by 3 minvalue 1 maxvalue 9223372036854775807;
create table t (id bigint default nid(), v integer, primary key (id), check (ok(v)));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
