-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
drop sequence s;
create sequence s start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
comment on sequence s is 'ids';
