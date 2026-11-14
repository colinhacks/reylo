drop type mood;
create type mood as enum('a', 'b', 'c');
-- DuckDB makes seq again, so it starts again at 1: where rows hold its values, start it past them
alter table t alter column id drop default;
drop sequence seq;
create sequence seq start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
alter table t alter column id set default nextval('seq');
alter table t alter column c set data type mood;
