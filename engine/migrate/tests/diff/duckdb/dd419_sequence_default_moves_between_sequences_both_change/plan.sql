-- DuckDB makes s1 again, so it starts again at 1: where rows hold its values, start it past them
alter table t alter column id drop default;
drop sequence s1;
create sequence s1 start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
-- DuckDB makes s2 again, so it starts again at 1: where rows hold its values, start it past them
alter table u alter column id drop default;
drop sequence s2;
create sequence s2 start with 1 increment by 3 minvalue 1 maxvalue 9223372036854775807;
alter table t alter column id set default nextval('s2');
alter table u alter column id set default nextval('s1');
