-- DuckDB makes zs again, so it starts again at 1: where rows hold its values, start it past them
alter table t alter column id drop default;
drop sequence zs;
create sequence zs start with 1 increment by 3 minvalue 1 maxvalue 9223372036854775807;
alter table t alter column id set default nextval('zs');
create table u (id bigint default nextval('zs'), primary key (id));
