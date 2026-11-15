-- DuckDB makes ids again, so it starts again at 1: where rows hold its values, start it past them
alter table a alter column id drop default;
alter table b alter column id drop default;
drop sequence ids;
create sequence ids start with 1 increment by 10 minvalue 1 maxvalue 9223372036854775807;
alter table a alter column id set default next_id();
alter table b alter column id set default next_id();
