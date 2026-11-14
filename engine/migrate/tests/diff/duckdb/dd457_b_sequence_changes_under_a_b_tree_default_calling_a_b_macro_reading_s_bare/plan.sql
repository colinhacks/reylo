-- DuckDB makes b.s again, so it starts again at 500: where rows hold its values, start it past them
alter table b.pages alter column k drop default;
drop sequence b.s;
create sequence b.s start with 500 increment by 2 minvalue 1 maxvalue 9223372036854775807;
alter table b.pages alter column k set default b.nx();
