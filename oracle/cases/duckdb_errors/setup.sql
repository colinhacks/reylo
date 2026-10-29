-- A statement DuckDB refuses is placed where DuckDB's caret points, and a PIVOT whose columns
-- are its data is refused.
create table logs (id integer primary key, level varchar not null, msg varchar);
