-- A check that reads a struct's field with a dot (`meta.a > 0`), which DuckDB's duckdb_constraints() binds as a column of the table and so refuses for the whole database: each table's keys are read from its own text, and bound a statement's rows as they do anywhere else.
create table items (id integer primary key, code varchar not null unique, region varchar not null, tier varchar, meta struct(a integer, b varchar), unique (region, tier), check (meta.a > 0));
create table tags (item_id integer not null references items (id), label varchar not null, primary key (item_id, label));
