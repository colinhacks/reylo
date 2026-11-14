-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
alter table u alter column id drop default;
create temporary table reylo_copy_t as select * from t;
drop table t;
drop sequence s;
drop macro valid;
create macro "valid" (c) as ((length(c) > 0));
create sequence s start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
create table t (id bigint default nextval('s'), code varchar, primary key (id), check ("valid"(code)));
insert into t (id, code) select id, code from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
alter table u alter column id set default nextval('s');
