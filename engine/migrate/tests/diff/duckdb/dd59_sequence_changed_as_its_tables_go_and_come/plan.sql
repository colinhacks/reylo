-- loses data: t1
drop table t1;
-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
alter table keep alter column id drop default;
drop sequence s;
create sequence s start with 1 increment by 5 minvalue 1 maxvalue 9223372036854775807;
alter table keep alter column id set default nextval('s');
create table t2 (id bigint default nextval('s'), b varchar);
