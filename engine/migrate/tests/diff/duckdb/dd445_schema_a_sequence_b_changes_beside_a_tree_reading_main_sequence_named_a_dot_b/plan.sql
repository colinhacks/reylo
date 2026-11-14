-- DuckDB makes a.b again, so it starts again at 1: where rows hold its values, start it past them
drop sequence a.b;
create sequence a.b start with 1 increment by 4 minvalue 1 maxvalue 9223372036854775807;
