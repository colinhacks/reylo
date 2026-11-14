-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
alter table t alter column pk drop default;
drop sequence s;
create sequence s start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
alter table t alter column pk set default nextval('s');
-- t.pk is renamed key, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table t rename column pk to key;
