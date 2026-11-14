-- DuckDB makes ids again, so it starts again at 1: where rows hold its values, start it past them
alter table t alter column id drop default;
drop macro next_id;
drop sequence ids;
create sequence ids start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
create macro next_id () as ((nextval('ids') + 0));
alter table t alter column id set default next_id();
