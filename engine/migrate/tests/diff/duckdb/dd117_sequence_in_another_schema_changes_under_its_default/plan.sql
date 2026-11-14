-- DuckDB makes b.ids again, so it starts again at 1: where rows hold its values, start it past them
alter table b.t alter column id drop default;
drop sequence b.ids;
create sequence b.ids start with 1 increment by 3 minvalue 1 maxvalue 9223372036854775807;
alter table b.t alter column id set default nextval('b.ids');
