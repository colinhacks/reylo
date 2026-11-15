-- DuckDB makes a.s again, so it starts again at 1: where rows hold its values, start it past them
alter table a.t alter column id drop default;
drop sequence a.s;
create sequence a.s start with 1 increment by 5 minvalue 1 maxvalue 9223372036854775807;
alter table a.t alter column id set default nextval('s');
